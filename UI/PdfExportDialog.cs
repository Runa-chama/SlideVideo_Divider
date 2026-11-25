using SlideVideoDivider.Core;

namespace SlideVideoDivider.UI;

public sealed class PdfExportDialog : Form
{
    private readonly RadioButton _combinedRadio = new() { Text = "スライド＋文字起こし（既定）", Checked = true };
    private readonly RadioButton _slidesOnlyRadio = new() { Text = "スライドのみ" };
    private readonly RadioButton _transcriptsOnlyRadio = new() { Text = "文字起こしのみ" };
    private readonly NumericUpDown _minimumFontNumeric = new()
    {
        Minimum = 5.0m,
        Maximum = 10.0m,
        Increment = 0.25m,
        DecimalPlaces = 2,
        Value = 6.5m
    };

    public PdfExportDialog(PdfExportMode mode, double minimumFontSize, bool darkMode)
    {
        Text = "PDF出力オプション";
        StartPosition = FormStartPosition.CenterParent;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(440, 250);
        Font = new Font("Yu Gothic", 9F, FontStyle.Regular, GraphicsUnit.Point);

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            Padding = new Padding(18)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 150));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        var title = new Label
        {
            Text = "PDFに含める内容",
            AutoSize = true,
            Font = new Font("Yu Gothic", 11F, FontStyle.Bold, GraphicsUnit.Point),
            Margin = new Padding(0, 0, 0, 10)
        };
        layout.Controls.Add(title, 0, 0);
        layout.SetColumnSpan(title, 2);

        _combinedRadio.Checked = mode == PdfExportMode.Combined;
        _slidesOnlyRadio.Checked = mode == PdfExportMode.SlidesOnly;
        _transcriptsOnlyRadio.Checked = mode == PdfExportMode.TranscriptsOnly;
        _minimumFontNumeric.Value = (decimal)Math.Clamp(minimumFontSize, 5.0, 10.0);

        layout.Controls.Add(_combinedRadio, 0, 1);
        layout.SetColumnSpan(_combinedRadio, 2);
        layout.Controls.Add(_slidesOnlyRadio, 0, 2);
        layout.SetColumnSpan(_slidesOnlyRadio, 2);
        layout.Controls.Add(_transcriptsOnlyRadio, 0, 3);
        layout.SetColumnSpan(_transcriptsOnlyRadio, 2);
        layout.Controls.Add(new Label { Text = "最小フォントサイズ", AutoSize = true, Margin = new Padding(0, 14, 8, 4) }, 0, 4);
        layout.Controls.Add(_minimumFontNumeric, 1, 4);

        var description = new Label
        {
            Text = "スライドのみ／文字起こしのみは、それぞれ1スライド＝1ページのPDFになります。",
            AutoSize = true,
            ForeColor = Color.Gray,
            Margin = new Padding(0, 8, 0, 12)
        };
        layout.Controls.Add(description, 0, 5);
        layout.SetColumnSpan(description, 2);

        var buttons = new FlowLayoutPanel
        {
            FlowDirection = FlowDirection.RightToLeft,
            AutoSize = true,
            Dock = DockStyle.Fill,
            Margin = new Padding(0)
        };
        var cancel = new Button { Text = "キャンセル", DialogResult = DialogResult.Cancel, Width = 100, Height = 30 };
        var ok = new Button { Text = "OK", DialogResult = DialogResult.OK, Width = 100, Height = 30 };
        Theme.StylePrimary(ok, darkMode);
        buttons.Controls.Add(cancel);
        buttons.Controls.Add(ok);
        layout.Controls.Add(buttons, 0, 6);
        layout.SetColumnSpan(buttons, 2);

        Controls.Add(layout);
        AcceptButton = ok;
        CancelButton = cancel;
        Theme.Apply(this, darkMode);
        Theme.StylePrimary(ok, darkMode);
    }

    public PdfExportOptions Options => new(
        _combinedRadio.Checked ? PdfExportMode.Combined :
        _slidesOnlyRadio.Checked ? PdfExportMode.SlidesOnly : PdfExportMode.TranscriptsOnly,
        (double)_minimumFontNumeric.Value);
}
