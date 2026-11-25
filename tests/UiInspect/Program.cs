using System.Reflection;
using SlideVideoDivider.Core;
using SlideVideoDivider.UI;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        var projectPath = args.Length > 0
            ? args[0]
            : Environment.GetEnvironmentVariable("SLIDEVIDEODIVIDER_TEST_PROJECT")
              ?? Directory.EnumerateFiles(AppContext.BaseDirectory, "*_project.json", SearchOption.AllDirectories).First();
        using var form = new MainForm(projectPath);
        form.Show();
        Application.DoEvents();
        Application.DoEvents();

        foreach (var name in new[] { "_startTimeTextBox", "_endTimeTextBox", "_transcriptCaptionLabel", "_transcriptTextBox", "_previewPictureBox", "_undoButton", "_redoButton" })
        {
            var field = typeof(MainForm).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!;
            var control = (Control)field.GetValue(form)!;
            Console.WriteLine($"{name}: visible={control.Visible}, bounds={control.Bounds}, parent={control.Parent?.Bounds}, text={control.Text}");
        }

        var projectField = typeof(MainForm).GetField("_project", BindingFlags.Instance | BindingFlags.NonPublic)!;
        var project = (SlideProject)projectField.GetValue(form)!;
        if (project.Slides.Count < 2)
        {
            throw new InvalidOperationException("UI history test requires at least two slides.");
        }

        project.Slides[0].Transcript = "A";
        project.Slides[1].Transcript = "B";
        var transcriptField = typeof(MainForm).GetField("_transcriptTextBox", BindingFlags.Instance | BindingFlags.NonPublic)!;
        ((TextBox)transcriptField.GetValue(form)!).Text = "A";
        Invoke(form, "MoveTranscript", 1);
        if (project.Slides[0].Transcript != string.Empty || project.Slides[1].Transcript != $"B{Environment.NewLine}A")
        {
            throw new InvalidOperationException("Move transcript failed.");
        }

        Invoke(form, "UndoEdit");
        if (project.Slides[0].Transcript != "A" || project.Slides[1].Transcript != "B")
        {
            throw new InvalidOperationException("Undo failed.");
        }

        Invoke(form, "RedoEdit");
        if (project.Slides[0].Transcript != string.Empty || project.Slides[1].Transcript != $"B{Environment.NewLine}A")
        {
            throw new InvalidOperationException("Redo failed.");
        }

        var originalImagePath = project.Slides[0].ImagePath;
        Invoke(form, "ReplaceImageFile", originalImagePath);
        var replacedImagePath = project.Slides[0].ImagePath;
        if (replacedImagePath == originalImagePath)
        {
            throw new InvalidOperationException("Image replacement failed.");
        }
        Invoke(form, "UndoEdit");
        if (project.Slides[0].ImagePath != originalImagePath)
        {
            throw new InvalidOperationException("Image replacement undo failed.");
        }
        Invoke(form, "RedoEdit");
        if (project.Slides[0].ImagePath != replacedImagePath)
        {
            throw new InvalidOperationException("Image replacement redo failed.");
        }

        typeof(MainForm).GetField("_dirty", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, false);
        using var pdfDialog = new PdfExportDialog(PdfExportMode.SlidesOnly, 7.25, true);
        if (pdfDialog.Options.Mode != PdfExportMode.SlidesOnly || pdfDialog.Options.MinimumFontSize != 7.25)
        {
            throw new InvalidOperationException("PDF options dialog failed.");
        }

        Console.WriteLine("UI INSPECT PASSED");
        form.Close();
    }

    private static void Invoke(MainForm form, string methodName, params object[] parameters)
    {
        typeof(MainForm).GetMethod(methodName, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, parameters);
    }
}
