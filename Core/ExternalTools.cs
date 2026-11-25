using System.Diagnostics;
using System.Globalization;
using System.Text.RegularExpressions;

namespace SlideTranscriber.Core;

public static class ExternalTools
{
    public static string? FindFfmpeg()
    {
        return FindTool("ffmpeg.exe");
    }

    public static string? FindFfprobe()
    {
        return FindTool("ffprobe.exe");
    }

    private static string? FindTool(string fileName)
    {
        var local = Path.Combine(AppContext.BaseDirectory, "tools", fileName);
        if (File.Exists(local))
        {
            return local;
        }

        var path = Environment.GetEnvironmentVariable("PATH") ?? string.Empty;
        foreach (var directory in path.Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries))
        {
            try
            {
                var candidate = Path.Combine(directory.Trim('"'), fileName);
                if (File.Exists(candidate))
                {
                    return candidate;
                }
            }
            catch
            {
                // Invalid PATH entries should not prevent tool discovery.
            }
        }

        return null;
    }

    public static async Task<double> ProbeDurationAsync(string videoPath, CancellationToken cancellationToken = default)
    {
        var ffprobe = FindFfprobe();
        if (ffprobe is not null)
        {
            var output = new List<string>();
            await RunAsync(
                ffprobe,
                ["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", videoPath],
                cancellationToken: cancellationToken,
                stdoutLine: line => output.Add(line));

            var text = string.Join("\n", output).Trim();
            if (double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out var duration) && duration > 0)
            {
                return duration;
            }
        }

        var ffmpeg = FindFfmpeg() ?? throw new InvalidOperationException(
            "FFmpegが見つかりません。FFmpegをインストールし、PATHに追加してください。");
        var error = new List<string>();
        await RunAsync(
            ffmpeg,
            ["-hide_banner", "-i", videoPath],
            cancellationToken: cancellationToken,
            stderrLine: line => error.Add(line),
            acceptNonZeroExit: true);

        var match = Regex.Match(string.Join("\n", error), @"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)");
        if (!match.Success)
        {
            throw new InvalidOperationException("動画の再生時間を取得できませんでした。");
        }

        var hours = int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture);
        var minutes = int.Parse(match.Groups[2].Value, CultureInfo.InvariantCulture);
        var seconds = double.Parse(match.Groups[3].Value, CultureInfo.InvariantCulture);
        return hours * 3600 + minutes * 60 + seconds;
    }

    public static async Task<List<double>> DetectSceneTimesAsync(
        string ffmpegPath,
        string videoPath,
        double duration,
        double threshold,
        double minimumSlideSeconds,
        IProgress<string>? status = null,
        IProgress<double>? progress = null,
        CancellationToken cancellationToken = default)
    {
        status?.Report("スライド切替を検出しています...");
        var times = new List<double> { 0 };
        var filter = $"select=gt(scene\\,{threshold.ToString(CultureInfo.InvariantCulture)}),metadata=print:file=-";

        await RunAsync(
            ffmpegPath,
            ["-hide_banner", "-loglevel", "error", "-i", videoPath, "-vf", filter, "-an", "-f", "null", "-"],
            cancellationToken: cancellationToken,
            stdoutLine: line =>
            {
                var match = Regex.Match(line, @"pts_time:([0-9]+(?:\.[0-9]+)?)");
                if (!match.Success ||
                    !double.TryParse(match.Groups[1].Value, NumberStyles.Float, CultureInfo.InvariantCulture, out var time))
                {
                    return;
                }

                if (time > 0.05 && time < duration - 0.05 && time - times[^1] >= minimumSlideSeconds)
                {
                    times.Add(time);
                    progress?.Report(Math.Min(100, time / duration * 100));
                }
            });

        return times;
    }

    public static async Task<List<SlideItem>> ExtractSlidesAsync(
        string ffmpegPath,
        string videoPath,
        IReadOnlyList<double> boundaries,
        double duration,
        string slidesDirectory,
        IProgress<string>? status = null,
        IProgress<double>? progress = null,
        CancellationToken cancellationToken = default)
    {
        Directory.CreateDirectory(slidesDirectory);
        var slides = new List<SlideItem>();

        for (var i = 0; i < boundaries.Count; i++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var start = boundaries[i];
            var end = i + 1 < boundaries.Count ? boundaries[i + 1] : duration;
            if (end <= start)
            {
                continue;
            }

            var offset = Math.Min(1.25, Math.Max(0.20, (end - start) * 0.12));
            var captureTime = Math.Min(end - 0.05, start + offset);
            var imagePath = Path.Combine(slidesDirectory, $"slide_{i + 1:000}.jpg");
            status?.Report($"スライド画像を抽出中... {i + 1}/{boundaries.Count}");

            await RunAsync(
                ffmpegPath,
                ["-y", "-v", "error", "-ss", captureTime.ToString(CultureInfo.InvariantCulture), "-i", videoPath,
                 "-frames:v", "1", "-q:v", "2", imagePath],
                cancellationToken: cancellationToken);

            if (!File.Exists(imagePath))
            {
                throw new InvalidOperationException($"スライド画像を抽出できませんでした: {captureTime:F2}秒");
            }

            slides.Add(new SlideItem
            {
                Number = slides.Count + 1,
                Start = start,
                End = end,
                ImagePath = imagePath
            });
            progress?.Report((i + 1) / (double)boundaries.Count * 100);
        }

        if (slides.Count == 0)
        {
            throw new InvalidOperationException("スライドを抽出できませんでした。");
        }
        return slides;
    }

    public static async Task ExtractSingleFrameAsync(
        string ffmpegPath,
        string videoPath,
        double time,
        string outputPath,
        CancellationToken cancellationToken = default)
    {
        await RunAsync(
            ffmpegPath,
            ["-y", "-v", "error", "-ss", time.ToString(CultureInfo.InvariantCulture), "-i", videoPath,
             "-frames:v", "1", "-q:v", "2", outputPath],
            cancellationToken: cancellationToken);

        if (!File.Exists(outputPath))
        {
            throw new InvalidOperationException("指定時刻のフレームを抽出できませんでした。");
        }
    }

    public static async Task ExtractAudioAsync(
        string ffmpegPath,
        string videoPath,
        string wavPath,
        IProgress<string>? status = null,
        CancellationToken cancellationToken = default)
    {
        status?.Report("音声を16kHzモノラルへ変換しています...");
        await RunAsync(
            ffmpegPath,
            ["-y", "-v", "error", "-i", videoPath, "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", wavPath],
            cancellationToken: cancellationToken);

        if (!File.Exists(wavPath))
        {
            throw new InvalidOperationException("音声を抽出できませんでした。動画に音声トラックがあるか確認してください。");
        }
    }

    public static async Task RunAsync(
        string fileName,
        IReadOnlyList<string> arguments,
        Action<string>? stdoutLine = null,
        Action<string>? stderrLine = null,
        CancellationToken cancellationToken = default,
        bool acceptNonZeroExit = false)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = fileName,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };

        foreach (var argument in arguments)
        {
            startInfo.ArgumentList.Add(argument);
        }

        using var process = new Process { StartInfo = startInfo };
        var errorTail = new Queue<string>();
        const int maxErrorLines = 30;
        process.OutputDataReceived += (_, e) =>
        {
            if (e.Data is not null)
            {
                stdoutLine?.Invoke(e.Data);
            }
        };
        process.ErrorDataReceived += (_, e) =>
        {
            if (e.Data is null)
            {
                return;
            }

            stderrLine?.Invoke(e.Data);
            errorTail.Enqueue(e.Data);
            while (errorTail.Count > maxErrorLines)
            {
                errorTail.Dequeue();
            }
        };

        if (!process.Start())
        {
            throw new InvalidOperationException($"プロセスを開始できませんでした: {fileName}");
        }

        process.BeginOutputReadLine();
        process.BeginErrorReadLine();

        try
        {
            await process.WaitForExitAsync(cancellationToken);
        }
        catch (OperationCanceledException)
        {
            try
            {
                process.Kill(entireProcessTree: true);
            }
            catch
            {
                // The process may already have exited.
            }
            throw;
        }

        if (process.ExitCode != 0 && !acceptNonZeroExit)
        {
            var message = string.Join("\n", errorTail).Trim();
            throw new InvalidOperationException(string.IsNullOrWhiteSpace(message)
                ? $"{Path.GetFileName(fileName)} failed (exit code {process.ExitCode})."
                : message);
        }
    }
}
