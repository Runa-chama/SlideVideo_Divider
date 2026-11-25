using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using PdfSharp.Drawing;
using PdfSharp.Pdf;

namespace SlideVideoDivider.Core;

public enum PdfExportMode
{
    Combined,
    SlidesOnly,
    TranscriptsOnly
}

public sealed record PdfExportOptions(PdfExportMode Mode, double MinimumFontSize)
{
    public static PdfExportOptions Default { get; } = new(PdfExportMode.Combined, 6.5);
}

public sealed class PdfLayoutException : Exception
{
    public IReadOnlyList<int> SlideNumbers { get; }
    public double MinimumFontSize { get; }

    public PdfLayoutException(IReadOnlyList<int> slideNumbers, double minimumFontSize)
        : base($"文字量が多く、最小フォントサイズ {minimumFontSize:0.00}pt でも1ページに収まらないスライドがあります。")
    {
        SlideNumbers = slideNumbers;
        MinimumFontSize = minimumFontSize;
    }
}

public static class PdfExporter
{
    private const double PageWidthPt = 595.2756;
    private const double PageHeightPt = 841.8898;
    private const double MarginPt = 28;
    private const double RenderScale = 2.0;

    public static async Task ExportAsync(
        SlideProject project,
        string outputPath,
        PdfExportOptions? options = null,
        IProgress<string>? status = null,
        IProgress<double>? progress = null,
        CancellationToken cancellationToken = default)
    {
        options ??= PdfExportOptions.Default;
        var minimumFontSize = Math.Clamp(options.MinimumFontSize, 5.0, 10.0);
        options = options with { MinimumFontSize = minimumFontSize };

        var fullOutputPath = Path.GetFullPath(outputPath);
        var tempDirectory = Path.Combine(Path.GetTempPath(), "SlideVideoDivider_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDirectory);
        var pageFiles = new List<string>();
        var overflowSlides = new List<int>();

        try
        {
            for (var i = 0; i < project.Slides.Count; i++)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var slide = project.Slides[i];
                status?.Report($"PDFページをレンダリング中... {i + 1}/{project.Slides.Count}");
                var pageFile = Path.Combine(tempDirectory, $"page_{i + 1:000}.jpg");

                if (!RenderPage(slide, pageFile, options))
                {
                    overflowSlides.Add(slide.Number);
                }

                pageFiles.Add(pageFile);
                progress?.Report((i + 1) / (double)project.Slides.Count * 90);
            }

            if (overflowSlides.Count > 0)
            {
                throw new PdfLayoutException(overflowSlides, minimumFontSize);
            }

            status?.Report("PDFを書き出しています...");
            using var document = new PdfDocument();
            document.Info.Title = Path.GetFileNameWithoutExtension(project.VideoName) + GetDocumentSuffix(options.Mode);
            document.Info.Author = "SlideVideoDivider";

            foreach (var pageFile in pageFiles)
            {
                var page = document.AddPage();
                page.Width = XUnit.FromPoint(PageWidthPt);
                page.Height = XUnit.FromPoint(PageHeightPt);
                using var image = XImage.FromFile(pageFile);
                using var graphics = XGraphics.FromPdfPage(page);
                graphics.DrawImage(image, 0, 0, page.Width.Point, page.Height.Point);
            }

            Directory.CreateDirectory(Path.GetDirectoryName(fullOutputPath)!);
            document.Save(fullOutputPath);
            progress?.Report(100);
        }
        finally
        {
            try
            {
                if (Directory.Exists(tempDirectory))
                {
                    Directory.Delete(tempDirectory, recursive: true);
                }
            }
            catch
            {
                // Temporary rendered pages can be cleaned up by Windows later.
            }
        }
    }

    private static bool RenderPage(SlideItem slide, string outputPath, PdfExportOptions options)
    {
        var width = (int)Math.Round(PageWidthPt * RenderScale);
        var height = (int)Math.Round(PageHeightPt * RenderScale);
        using var bitmap = new Bitmap(width, height, PixelFormat.Format24bppRgb);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.Clear(Color.White);
        graphics.SmoothingMode = SmoothingMode.HighQuality;
        graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
        graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
        graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        graphics.ScaleTransform((float)RenderScale, (float)RenderScale);

        var contentWidth = PageWidthPt - MarginPt * 2;
        var fits = true;

        switch (options.Mode)
        {
            case PdfExportMode.Combined:
                var halfHeight = PageHeightPt / 2;
                DrawSlideImage(graphics, slide,
                    new RectangleF((float)MarginPt, 18, (float)contentWidth, (float)(halfHeight - 36)));
                using (var divider = new Pen(Color.FromArgb(220, 224, 230), 0.8f))
                {
                    graphics.DrawLine(divider, (float)MarginPt, (float)halfHeight,
                        (float)(PageWidthPt - MarginPt), (float)halfHeight);
                }
                DrawTranscriptHeader(graphics, slide, (float)(halfHeight + 14));
                fits = DrawTranscript(graphics, slide, halfHeight + 36, PageHeightPt - MarginPt,
                    contentWidth, options.MinimumFontSize);
                break;

            case PdfExportMode.SlidesOnly:
                DrawSlideImage(graphics, slide,
                    new RectangleF((float)MarginPt, 30, (float)contentWidth, (float)(PageHeightPt - 100)));
                DrawSlideFooter(graphics, slide);
                break;

            case PdfExportMode.TranscriptsOnly:
                DrawTranscriptHeader(graphics, slide, (float)MarginPt);
                fits = DrawTranscript(graphics, slide, MarginPt + 42, PageHeightPt - MarginPt,
                    contentWidth, options.MinimumFontSize);
                break;
        }

        if (!fits)
        {
            return false;
        }

        var encoder = GetJpegEncoder();
        using var encoderParameters = new EncoderParameters(1);
        encoderParameters.Param[0] = new EncoderParameter(Encoder.Quality, 92L);
        bitmap.Save(outputPath, encoder, encoderParameters);
        return true;
    }

    private static void DrawSlideImage(Graphics graphics, SlideItem slide, RectangleF area)
    {
        if (File.Exists(slide.ImagePath))
        {
            using var slideImage = Image.FromFile(slide.ImagePath);
            var ratio = Math.Min(area.Width / slideImage.Width, area.Height / slideImage.Height);
            var drawWidth = slideImage.Width * ratio;
            var drawHeight = slideImage.Height * ratio;
            var x = area.X + (area.Width - drawWidth) / 2;
            var y = area.Y + (area.Height - drawHeight) / 2;
            graphics.DrawImage(slideImage, new RectangleF((float)x, (float)y, (float)drawWidth, (float)drawHeight));
            using var border = new Pen(Color.FromArgb(210, 214, 220), 0.6f);
            graphics.DrawRectangle(border, (float)x, (float)y, (float)drawWidth, (float)drawHeight);
            return;
        }

        using var missingFont = new Font(JapaneseFontFamily(), 10, FontStyle.Regular, GraphicsUnit.Point);
        using var missingBrush = new SolidBrush(Color.FromArgb(150, 150, 150));
        var message = "スライド画像が見つかりません";
        var messageSize = graphics.MeasureString(message, missingFont);
        graphics.DrawString(message, missingFont, missingBrush,
            area.X + (area.Width - messageSize.Width) / 2,
            area.Y + (area.Height - messageSize.Height) / 2);
    }

    private static void DrawTranscriptHeader(Graphics graphics, SlideItem slide, float y)
    {
        using var headerFont = new Font(JapaneseFontFamily(), 9, FontStyle.Bold, GraphicsUnit.Point);
        using var headerBrush = new SolidBrush(Color.FromArgb(80, 86, 96));
        graphics.DrawString("文字起こし", headerFont, headerBrush, (float)MarginPt, y);

        var timeText = $"No. {slide.Number:D2}    {FormatTime(slide.Start)} – {FormatTime(slide.End)}";
        using var timeFont = new Font(JapaneseFontFamily(), 8, FontStyle.Regular, GraphicsUnit.Point);
        var timeSize = graphics.MeasureString(timeText, timeFont);
        graphics.DrawString(timeText, timeFont, headerBrush,
            (float)(PageWidthPt - MarginPt - timeSize.Width), y + 1);
    }

    private static void DrawSlideFooter(Graphics graphics, SlideItem slide)
    {
        var text = $"No. {slide.Number:D2}    {FormatTime(slide.Start)} – {FormatTime(slide.End)}";
        using var font = new Font(JapaneseFontFamily(), 8, FontStyle.Regular, GraphicsUnit.Point);
        using var brush = new SolidBrush(Color.FromArgb(110, 116, 126));
        var size = graphics.MeasureString(text, font);
        graphics.DrawString(text, font, brush,
            (float)((PageWidthPt - size.Width) / 2), (float)(PageHeightPt - MarginPt - 4));
    }

    private static bool DrawTranscript(
        Graphics graphics,
        SlideItem slide,
        double bodyTop,
        double bodyBottom,
        double contentWidth,
        double minimumFontSize)
    {
        var availableHeight = bodyBottom - bodyTop;
        var transcript = TranscriptAligner.NormalizeTranscript(slide.Transcript);
        if (string.IsNullOrWhiteSpace(transcript))
        {
            transcript = "（文字起こしなし）";
        }

        var format = StringFormat.GenericTypographic;
        List<string>? bestLines = null;
        Font? bestFont = null;
        for (var size = 10.5; size >= minimumFontSize - 0.001; size -= 0.25)
        {
            var font = new Font(JapaneseFontFamily(), (float)size, FontStyle.Regular, GraphicsUnit.Point);
            var lines = WrapText(graphics, transcript, font, (float)contentWidth, format);
            var lineHeight = size * 1.48;
            if (lines.Count * lineHeight <= availableHeight)
            {
                bestLines = lines;
                bestFont = font;
                break;
            }
            font.Dispose();
        }

        if (bestLines is null || bestFont is null)
        {
            return false;
        }

        using (bestFont)
        using (var bodyBrush = new SolidBrush(Color.FromArgb(32, 36, 43)))
        {
            var lineHeight = bestFont.Size * 1.48f;
            var y = bodyTop;
            foreach (var line in bestLines)
            {
                graphics.DrawString(line, bestFont, bodyBrush, (float)MarginPt, (float)y, format);
                y += lineHeight;
            }
        }
        return true;
    }

    private static string GetDocumentSuffix(PdfExportMode mode) => mode switch
    {
        PdfExportMode.SlidesOnly => " スライド",
        PdfExportMode.TranscriptsOnly => " 文字起こし",
        _ => " スライドノート"
    };

    private static List<string> WrapText(Graphics graphics, string text, Font font, float maxWidth, StringFormat format)
    {
        var lines = new List<string>();
        var paragraphs = text.Replace("\r\n", "\n").Replace('\r', '\n').Split('\n');
        foreach (var paragraph in paragraphs)
        {
            if (string.IsNullOrEmpty(paragraph))
            {
                lines.Add(string.Empty);
                continue;
            }

            var line = string.Empty;
            foreach (var character in paragraph)
            {
                var candidate = line + character;
                if (!string.IsNullOrEmpty(line) && graphics.MeasureString(candidate, font, int.MaxValue, format).Width > maxWidth)
                {
                    lines.Add(line);
                    line = character.ToString();
                }
                else
                {
                    line = candidate;
                }
            }
            lines.Add(line);
        }

        ApplyKinsoku(lines);
        return lines;
    }

    private static void ApplyKinsoku(List<string> lines)
    {
        const string closing = "、。，．！？）］】」』】〉》〕｝〕」』）";
        const string opening = "（［【「『《〈〔｛「『（";
        for (var i = 0; i < lines.Count - 1; i++)
        {
            if (!string.IsNullOrEmpty(lines[i]) && !string.IsNullOrEmpty(lines[i + 1]) &&
                closing.Contains(lines[i + 1][0]) && lines[i].Length > 1)
            {
                lines[i + 1] = lines[i][^1] + lines[i + 1];
                lines[i] = lines[i][..^1];
            }

            if (!string.IsNullOrEmpty(lines[i]) && !string.IsNullOrEmpty(lines[i + 1]) &&
                opening.Contains(lines[i][^1]))
            {
                lines[i + 1] = lines[i][^1] + lines[i + 1];
                lines[i] = lines[i][..^1];
            }
        }
    }

    private static string JapaneseFontFamily()
    {
        foreach (var name in new[] { "Yu Gothic", "Meiryo", "MS Gothic" })
        {
            using var test = new Font(name, 9);
            if (test.Name == name)
            {
                return name;
            }
        }
        return "Arial";
    }

    private static ImageCodecInfo GetJpegEncoder()
    {
        foreach (var codec in ImageCodecInfo.GetImageEncoders())
        {
            if (codec.FormatID == ImageFormat.Jpeg.Guid)
            {
                return codec;
            }
        }
        throw new InvalidOperationException("JPEG encoder was not found.");
    }

    private static string FormatTime(double seconds)
    {
        seconds = Math.Max(0, seconds);
        var time = TimeSpan.FromSeconds(seconds);
        return time.Hours > 0
            ? $"{(int)time.TotalHours}:{time.Minutes:D2}:{time.Seconds:D2}"
            : $"{time.Minutes}:{time.Seconds:D2}";
    }
}
