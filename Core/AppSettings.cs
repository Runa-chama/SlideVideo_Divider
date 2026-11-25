using System.Text.Json;

namespace SlideVideoDivider.Core;

public sealed class AppSettings
{
    public bool DarkMode { get; set; } = true;
    public bool UseGpu { get; set; } = true;
    public string OutputDirectory { get; set; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyVideos), "SlideVideoDivider");
    public string ModelDirectory { get; set; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideVideoDivider", "models");
    public double SceneThreshold { get; set; } = 0.12;
    public double MinimumSlideSeconds { get; set; } = 1.5;
    public string InitialPrompt { get; set; } = "工学、数学、物理学、制御工学、専門用語。";
    public int ModelIndex { get; set; }
    public double PdfMinimumFontSize { get; set; } = 6.5;
    public string PdfExportMode { get; set; } = "Combined";

    public static string SettingsPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideVideoDivider", "settings.json");

    private static string LegacySettingsPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideTranscriber", "settings.json");

    public static AppSettings Load()
    {
        try
        {
            var path = File.Exists(SettingsPath) ? SettingsPath : LegacySettingsPath;
            if (!File.Exists(path))
            {
                return new AppSettings();
            }

            var settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(path)) ?? new AppSettings();
            settings.PdfMinimumFontSize = Math.Clamp(settings.PdfMinimumFontSize, 5.0, 10.0);
            if (settings.PdfExportMode is not ("Combined" or "SlidesOnly" or "TranscriptsOnly"))
            {
                settings.PdfExportMode = "Combined";
            }

            if (path == LegacySettingsPath)
            {
                settings.Save();
            }
            return settings;
        }
        catch
        {
            return new AppSettings();
        }
    }

    public void Save()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
        File.WriteAllText(SettingsPath, JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }));
    }
}
