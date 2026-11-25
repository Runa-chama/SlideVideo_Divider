namespace SlideTranscriber.Core;

public static class TranscriptAligner
{
    public static void AssignTranscripts(SlideProject project)
    {
        foreach (var slide in project.Slides)
        {
            slide.Transcript = string.Empty;
        }

        foreach (var segment in project.Segments.OrderBy(s => s.Start))
        {
            var slide = FindBestSlide(project.Slides, segment.Start, segment.End);
            if (slide is null)
            {
                continue;
            }

            var text = segment.Text.Trim();
            if (string.IsNullOrEmpty(text))
            {
                continue;
            }

            slide.Transcript = string.IsNullOrWhiteSpace(slide.Transcript)
                ? text
                : slide.Transcript + Environment.NewLine + Environment.NewLine + text;
        }
    }

    private static SlideItem? FindBestSlide(IReadOnlyList<SlideItem> slides, double start, double end)
    {
        SlideItem? best = null;
        var bestOverlap = 0.0;
        foreach (var slide in slides)
        {
            var overlap = Math.Min(end, slide.End) - Math.Max(start, slide.Start);
            if (overlap <= 0)
            {
                continue;
            }

            if (best is null || overlap > bestOverlap)
            {
                best = slide;
                bestOverlap = overlap;
            }
        }

        if (best is not null)
        {
            return best;
        }

        foreach (var slide in slides)
        {
            if (start >= slide.Start && start < slide.End)
            {
                return slide;
            }
        }

        return start >= (slides.LastOrDefault()?.End ?? 0) ? slides.LastOrDefault() : slides.FirstOrDefault();
    }
}
