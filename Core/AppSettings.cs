using System.Text.Json;

namespace SlideTranscriber.Core;

public sealed class AppSettings
{
    public bool DarkMode { get; set; } = true;
    public bool UseGpu { get; set; } = true;
    public string OutputDirectory { get; set; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyVideos), "SlideTranscriber");
    public string ModelDirectory { get; set; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideTranscriber", "models");
    public double SceneThreshold { get; set; } = 0.12;
    public double MinimumSlideSeconds { get; set; } = 1.5;
    public string InitialPrompt { get; set; } = "工学、数学、物理学、制御工学、専門用語。";
    public int ModelIndex { get; set; }

    public static string SettingsPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideTranscriber", "settings.json");

    public static AppSettings Load()
    {
        try
        {
            if (!File.Exists(SettingsPath))
            {
                return new AppSettings();
            }
            return JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(SettingsPath)) ?? new AppSettings();
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
