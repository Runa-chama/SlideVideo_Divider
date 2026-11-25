using System.Globalization;
using Whisper.net;
using Whisper.net.Ggml;
using Whisper.net.LibraryLoader;

namespace SlideTranscriber.Core;

public static class TranscriptionService
{
    public static async Task EnsureModelAsync(
        ModelChoice model,
        string modelPath,
        IProgress<string>? status = null,
        IProgress<long>? downloadedBytes = null,
        CancellationToken cancellationToken = default)
    {
        if (File.Exists(modelPath) && new FileInfo(modelPath).Length > 1024 * 1024)
        {
            return;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(modelPath) ?? AppContext.BaseDirectory);
        var partialPath = modelPath + ".part";
        status?.Report($"音声認識モデルをダウンロード中... {model.DisplayName}");

        await using var source = await WhisperGgmlDownloader.Default.GetGgmlModelAsync(
            model.Type,
            model.Quantization,
            cancellationToken);
        await using var target = File.Create(partialPath);
        var buffer = new byte[1024 * 128];
        long total = 0;
        int read;
        while ((read = await source.ReadAsync(buffer, cancellationToken)) > 0)
        {
            await target.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
            total += read;
            downloadedBytes?.Report(total);
            status?.Report($"音声認識モデルをダウンロード中... {total / 1024.0 / 1024.0:F0} MB");
        }

        await target.DisposeAsync();
        if (total <= 1024 * 1024)
        {
            throw new InvalidOperationException("音声認識モデルのダウンロードに失敗しました。");
        }

        File.Move(partialPath, modelPath, overwrite: true);
    }

    public static async Task<List<TranscriptSegment>> TranscribeAsync(
        string wavPath,
        string modelPath,
        bool useGpu,
        string initialPrompt,
        double overallStartPercent,
        double overallEndPercent,
        IProgress<string>? status = null,
        IProgress<double>? overallProgress = null,
        CancellationToken cancellationToken = default)
    {
        if (!File.Exists(modelPath))
        {
            throw new FileNotFoundException("音声認識モデルが見つかりません。先にモデルをダウンロードしてください。", modelPath);
        }

        RuntimeOptions.RuntimeLibraryOrder = useGpu
            ? [RuntimeLibrary.Cuda, RuntimeLibrary.Cpu, RuntimeLibrary.CpuNoAvx]
            : [RuntimeLibrary.Cpu, RuntimeLibrary.CpuNoAvx];

        status?.Report("音声認識モデルを読み込んでいます...");
        using var factory = WhisperFactory.FromPath(modelPath);
        var runtimeName = RuntimeOptions.LoadedLibrary?.ToString() ?? (useGpu ? "GPU/自動" : "CPU");
        status?.Report($"文字起こし中... 使用ランタイム: {runtimeName}");

        var builder = factory.CreateBuilder()
            .WithLanguage("ja")
            .WithNoContext()
            .WithThreads(Math.Min(8, Environment.ProcessorCount))
            .WithProgressHandler(percent =>
            {
                var mapped = overallStartPercent + (overallEndPercent - overallStartPercent) * percent / 100.0;
                overallProgress?.Report(mapped);
                status?.Report($"文字起こし中... {percent}%");
            });

        if (!string.IsNullOrWhiteSpace(initialPrompt))
        {
            builder.WithPrompt(initialPrompt.Trim()).WithCarryInitialPrompt();
        }

        await using var processor = builder.Build();
        var segments = new List<TranscriptSegment>();
        using var audioStream = File.OpenRead(wavPath);

        await foreach (var segment in processor.ProcessAsync(audioStream, cancellationToken))
        {
            var text = segment.Text.Trim();
            if (string.IsNullOrWhiteSpace(text))
            {
                continue;
            }

            segments.Add(new TranscriptSegment
            {
                Start = segment.Start.TotalSeconds,
                End = segment.End.TotalSeconds,
                Text = text
            });
        }

        if (segments.Count == 0)
        {
            throw new InvalidOperationException("文字起こし結果が空でした。音声の有無・言語設定を確認してください。");
        }

        return segments;
    }

    public static string FormatBytes(long bytes)
    {
        return bytes switch
        {
            >= 1024L * 1024 * 1024 => $"{bytes / 1024.0 / 1024 / 1024:F2} GB",
            >= 1024L * 1024 => $"{bytes / 1024.0 / 1024:F0} MB",
            _ => $"{bytes / 1024.0:F0} KB"
        };
    }
}
