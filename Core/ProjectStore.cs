using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace SlideTranscriber.Core;

public static class ProjectStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNameCaseInsensitive = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };

    public static void Save(string path, SlideProject project)
    {
        var fullPath = Path.GetFullPath(path);
        var directory = Path.GetDirectoryName(fullPath)!;
        Directory.CreateDirectory(directory);
        project.ProjectPath = fullPath;
        project.Renumber();

        var document = new SlideProject
        {
            Version = project.Version,
            VideoPath = project.VideoPath,
            VideoName = project.VideoName,
            ModelName = project.ModelName,
            ModelPath = project.ModelPath,
            CreatedAt = project.CreatedAt,
            Segments = project.Segments,
            Slides = project.Slides.Select(slide => new SlideItem
            {
                Number = slide.Number,
                Start = slide.Start,
                End = slide.End,
                Transcript = slide.Transcript,
                ImagePath = string.IsNullOrWhiteSpace(slide.ImagePath) || !Path.IsPathRooted(slide.ImagePath)
                    ? slide.ImagePath
                    : Path.GetRelativePath(directory, slide.ImagePath)
            }).ToList()
        };

        var json = JsonSerializer.Serialize(document, JsonOptions);
        File.WriteAllText(fullPath, json, new UTF8Encoding(encoderShouldEmitUTF8Identifier: true));
    }

    public static SlideProject Load(string path)
    {
        var fullPath = Path.GetFullPath(path);
        var json = File.ReadAllText(fullPath);
        var project = JsonSerializer.Deserialize<SlideProject>(json, JsonOptions)
                     ?? throw new InvalidDataException("プロジェクトファイルを読み込めませんでした。");
        project.ProjectPath = fullPath;

        var directory = Path.GetDirectoryName(fullPath)!;
        foreach (var slide in project.Slides)
        {
            if (string.IsNullOrWhiteSpace(slide.ImagePath))
            {
                continue;
            }

            slide.ImagePath = Path.IsPathRooted(slide.ImagePath)
                ? slide.ImagePath
                : Path.GetFullPath(Path.Combine(directory, slide.ImagePath));
        }

        project.Renumber();
        return project;
    }

    public static string GetDefaultProjectPath(string outputDirectory, string videoPath)
    {
        var name = Path.GetFileNameWithoutExtension(videoPath);
        return Path.Combine(outputDirectory, name + "_project.json");
    }
}
