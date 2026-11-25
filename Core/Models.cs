using System.Text.Json.Serialization;
using Whisper.net.Ggml;

namespace SlideTranscriber.Core;

public sealed class TranscriptSegment
{
    public double Start { get; set; }
    public double End { get; set; }
    public string Text { get; set; } = string.Empty;
}

public sealed class SlideItem
{
    public int Number { get; set; }
    public double Start { get; set; }
    public double End { get; set; }
    public string ImagePath { get; set; } = string.Empty;
    public string Transcript { get; set; } = string.Empty;

    [JsonIgnore]
    public double Duration => Math.Max(0, End - Start);
}

public sealed class SlideProject
{
    public int Version { get; set; } = 1;
    public string VideoPath { get; set; } = string.Empty;
    public string VideoName { get; set; } = string.Empty;
    public string ModelName { get; set; } = string.Empty;
    public string ModelPath { get; set; } = string.Empty;
    public DateTime CreatedAt { get; set; } = DateTime.Now;
    public List<SlideItem> Slides { get; set; } = [];
    public List<TranscriptSegment> Segments { get; set; } = [];

    [JsonIgnore]
    public string? ProjectPath { get; set; }

    [JsonIgnore]
    public string ProjectDirectory => ProjectPath is null
        ? AppContext.BaseDirectory
        : Path.GetDirectoryName(ProjectPath) ?? AppContext.BaseDirectory;

    public void Renumber()
    {
        for (var i = 0; i < Slides.Count; i++)
        {
            Slides[i].Number = i + 1;
        }
    }
}

public sealed class ModelChoice
{
    public required string DisplayName { get; set; }
    public required GgmlType Type { get; set; }
    public required QuantizationType Quantization { get; set; }
    public required string FileName { get; set; }
    public string Description { get; set; } = string.Empty;

    public override string ToString() => DisplayName;
}

public static class ModelCatalog
{
    public static IReadOnlyList<ModelChoice> Choices { get; } =
    [
        new()
        {
            DisplayName = "large-v3-turbo / Q8（推奨）",
            Type = GgmlType.LargeV3Turbo,
            Quantization = QuantizationType.Q8_0,
            FileName = "ggml-large-v3-turbo-q8_0.bin",
            Description = "速度と精度のバランスが良い日本語講義向けモデル"
        },
        new()
        {
            DisplayName = "large-v3 / Q8（高精度）",
            Type = GgmlType.LargeV3,
            Quantization = QuantizationType.Q8_0,
            FileName = "ggml-large-v3-q8_0.bin",
            Description = "最も高精度だが処理時間とVRAM使用量が大きい"
        },
        new()
        {
            DisplayName = "medium / Q5",
            Type = GgmlType.Medium,
            Quantization = QuantizationType.Q5_0,
            FileName = "ggml-medium-q5_0.bin",
            Description = "中量級。低性能GPU向け"
        },
        new()
        {
            DisplayName = "small / Q5",
            Type = GgmlType.Small,
            Quantization = QuantizationType.Q5_0,
            FileName = "ggml-small-q5_0.bin",
            Description = "軽量。動作確認や短い動画向け"
        },
        new()
        {
            DisplayName = "tiny / Q5（テスト）",
            Type = GgmlType.Tiny,
            Quantization = QuantizationType.Q5_0,
            FileName = "ggml-tiny-q5_0.bin",
            Description = "最小モデル。精度は低い"
        }
    ];

    public static ModelChoice Default => Choices[0];
}
