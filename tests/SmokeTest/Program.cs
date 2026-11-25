using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using SlideVideoDivider.Core;

if (args.Contains("--capture-ui") || args.Contains("--capture-notes"))
{
    var captureNotes = args.Contains("--capture-notes");
    var exe = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "..", "bin", "Release", "net10.0-windows", "SlideVideoDivider.exe"));
    var initialProjectPath = Directory.EnumerateFiles(AppContext.BaseDirectory, "*_project.json", SearchOption.AllDirectories).FirstOrDefault();
    using var process = captureNotes && initialProjectPath is not null
        ? Process.Start(exe, initialProjectPath) ?? throw new InvalidOperationException("Failed to start app")
        : Process.Start(exe) ?? throw new InvalidOperationException("Failed to start app");
    process.WaitForInputIdle(15000);
    await Task.Delay(900);
    Console.WriteLine($"App pid={process.Id}, exited={process.HasExited}, hwnd={process.MainWindowHandle}, title={process.MainWindowTitle}");
    NativeMethods.SetForegroundWindow(process.MainWindowHandle);
    NativeMethods.SwitchToThisWindow(process.MainWindowHandle, true);
    await Task.Delay(300);
    NativeMethods.GetWindowRect(process.MainWindowHandle, out var rect);
    using var bitmap = new Bitmap(rect.Right - rect.Left, rect.Bottom - rect.Top);
    using var graphics = Graphics.FromImage(bitmap);
    var hdc = graphics.GetHdc();
    NativeMethods.PrintWindow(process.MainWindowHandle, hdc, 2);
    graphics.ReleaseHdc(hdc);
    Console.WriteLine($"Pixels: header={bitmap.GetPixel(300, 60).ToArgb():X8}, content={bitmap.GetPixel(300, 120).ToArgb():X8}, body={bitmap.GetPixel(300, 300).ToArgb():X8}");
    var capturePath = Path.Combine(Path.GetTempPath(), captureNotes ? "slide-transcriber-notes.png" : "slide-transcriber-ui.png");
    bitmap.Save(capturePath, ImageFormat.Png);
    if (captureNotes)
    {
        using var crop = bitmap.Clone(new Rectangle(760, 75, 390, 150), bitmap.PixelFormat);
        crop.Save(Path.Combine(Path.GetTempPath(), "slide-transcriber-notes-header.png"), ImageFormat.Png);
    }
    process.CloseMainWindow();
    await process.WaitForExitAsync();
    Console.WriteLine(capturePath);
    return;
}

var videoArgumentIndex = Array.IndexOf(args, "--video");
var videoPath = videoArgumentIndex >= 0 && videoArgumentIndex + 1 < args.Length
    ? args[videoArgumentIndex + 1]
    : Environment.GetEnvironmentVariable("SLIDEVIDEODIVIDER_TEST_VIDEO");
if (string.IsNullOrWhiteSpace(videoPath) || !File.Exists(videoPath))
{
    Console.WriteLine("No test video specified. Use --video <path> or set SLIDEVIDEODIVIDER_TEST_VIDEO.");
    return;
}
var video = Path.GetFullPath(videoPath);
var ffmpeg = ExternalTools.FindFfmpeg() ?? throw new InvalidOperationException("FFmpeg not found");
var output = Path.Combine(AppContext.BaseDirectory, "smoke-output");
Directory.CreateDirectory(output);

Console.WriteLine($"Video: {video}");
var duration = await ExternalTools.ProbeDurationAsync(video);
Console.WriteLine($"Duration: {duration:F2}s");

var boundaries = await ExternalTools.DetectSceneTimesAsync(ffmpeg, video, duration, 0.12, 1.5);
Console.WriteLine($"Boundaries ({boundaries.Count}): {string.Join(", ", boundaries.Select(x => x.ToString("F2")))}");

var slides = await ExternalTools.ExtractSlidesAsync(
    ffmpeg, video, boundaries, duration, Path.Combine(output, "slides"));
Console.WriteLine($"Slides extracted: {slides.Count}");

var project = new SlideProject
{
    VideoPath = video,
    VideoName = Path.GetFileName(video),
    Slides = slides,
    Segments =
    [
        new TranscriptSegment { Start = 0, End = 10, Text = "これは動作確認用の文字起こしです。日本語の折り返しとレイアウトを確認します。" }
    ]
};
TranscriptAligner.AssignTranscripts(project);
if (TranscriptAligner.NormalizeTranscript("一行目\n\n二行目\n \n三行目") != $"一行目{Environment.NewLine}二行目{Environment.NewLine}三行目")
{
    throw new InvalidOperationException("Transcript blank-line normalization failed.");
}

var history = new EditHistory();
var historyValue = 0;
history.Push(new EditAction
{
    Description = "test",
    Undo = () => historyValue--,
    Redo = () => historyValue++
});
historyValue++;
history.Undo();
history.Redo();
if (historyValue != 1 || !history.CanUndo || history.CanRedo)
{
    throw new InvalidOperationException("Edit history failed.");
}

var projectPath = ProjectStore.GetDefaultProjectPath(output, video);
ProjectStore.Save(projectPath, project);
var loaded = ProjectStore.Load(projectPath);
Console.WriteLine($"Project round-trip: {loaded.Slides.Count} slides");

foreach (var mode in Enum.GetValues<PdfExportMode>())
{
    var modePath = Path.Combine(output, $"smoke_{mode}.pdf");
    await PdfExporter.ExportAsync(project, modePath, new PdfExportOptions(mode, 6.5));
    Console.WriteLine($"PDF ({mode}): {modePath} ({new FileInfo(modePath).Length / 1024.0:F1} KB)");
}

var originalTranscript = project.Slides[0].Transcript;
project.Slides[0].Transcript = new string('あ', 15000);
try
{
    await PdfExporter.ExportAsync(project, Path.Combine(output, "overflow.pdf"),
        new PdfExportOptions(PdfExportMode.TranscriptsOnly, 5.0));
    throw new InvalidOperationException("Expected PDF layout overflow was not detected.");
}
catch (PdfLayoutException ex)
{
    if (ex.MinimumFontSize != 5.0 || !ex.SlideNumbers.Contains(project.Slides[0].Number))
    {
        throw new InvalidOperationException("PDF layout overflow reporting failed.");
    }
}
finally
{
    project.Slides[0].Transcript = originalTranscript;
}

if (args.Contains("--whisper") || args.Contains("--full-whisper"))
{
    var model = ModelCatalog.Choices[^1];
    var modelPath = Path.Combine(output, model.FileName);
    await TranscriptionService.EnsureModelAsync(model, modelPath);
    var wavPath = Path.Combine(output, args.Contains("--full-whisper") ? "full.wav" : "sample.wav");
    if (args.Contains("--full-whisper"))
    {
        await ExternalTools.ExtractAudioAsync(ffmpeg, video, wavPath);
    }
    else
    {
        await ExternalTools.RunAsync(
            ffmpeg,
            ["-y", "-v", "error", "-i", video, "-t", "15", "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", wavPath]);
    }

    var segments = await TranscriptionService.TranscribeAsync(
        wavPath, modelPath, useGpu: true, initialPrompt: "工学、数学、物理", 0, 100);
    Console.WriteLine($"Whisper segments: {segments.Count}");
    foreach (var segment in segments.Take(5))
    {
        Console.WriteLine($"[{segment.Start:F2}-{segment.End:F2}] {segment.Text}");
    }

    if (args.Contains("--full-whisper"))
    {
        project.Segments = segments;
        TranscriptAligner.AssignTranscripts(project);
        Console.WriteLine($"Slides with text: {project.Slides.Count(s => !string.IsNullOrWhiteSpace(s.Transcript))}/{project.Slides.Count}");
        Console.WriteLine($"Max transcript length: {project.Slides.Max(s => s.Transcript.Length)}");
        if (project.Slides.Any(s => s.Transcript.Contains("\n\n", StringComparison.Ordinal)))
        {
            throw new InvalidOperationException("Transcript contains blank lines after alignment.");
        }
    }
}

Console.WriteLine("SMOKE TEST PASSED");

internal static partial class NativeMethods
{
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBmp, uint nFlags);

    [DllImport("user32.dll")]
    public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count);

    [DllImport("user32.dll")]
    public static extern void SwitchToThisWindow(IntPtr hWnd, bool altTab);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out Rect lpRect);

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }
}
