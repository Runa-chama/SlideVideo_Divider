using SlideVideoDivider.UI;

namespace SlideVideoDivider;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        Application.ThreadException += (_, e) => ShowFatalError(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => ShowFatalError(e.ExceptionObject as Exception);
        var projectPath = args.FirstOrDefault(arg => arg.EndsWith(".json", StringComparison.OrdinalIgnoreCase) && File.Exists(arg));
        Application.Run(new MainForm(projectPath));
    }

    private static void ShowFatalError(Exception? exception)
    {
        if (exception is null)
        {
            return;
        }

        try
        {
            var directory = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SlideVideoDivider");
            Directory.CreateDirectory(directory);
            File.WriteAllText(Path.Combine(directory, "error.log"), exception.ToString());
        }
        catch
        {
            // Logging must never hide the original error dialog.
        }

        MessageBox.Show(exception.Message, "SlideVideoDivider エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
    }
}
