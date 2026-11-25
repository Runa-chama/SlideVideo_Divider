using System.Reflection;
using SlideTranscriber.UI;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        var projectPath = args.Length > 0 ? args[0] : Directory.EnumerateFiles(AppContext.BaseDirectory, "*_project.json", SearchOption.AllDirectories).First();
        using var form = new MainForm(projectPath);
        form.Show();
        Application.DoEvents();

        foreach (var name in new[] { "_startTimeTextBox", "_endTimeTextBox", "_transcriptCaptionLabel", "_transcriptTextBox", "_previewPictureBox" })
        {
            var field = typeof(MainForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!;
            var control = (Control)field.GetValue(form)!;
            Console.WriteLine($"{name}: visible={control.Visible}, bounds={control.Bounds}, parent={control.Parent?.Bounds}, text={control.Text}");
        }

        form.Close();
    }
}
