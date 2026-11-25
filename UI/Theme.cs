using System.Drawing.Drawing2D;

namespace SlideVideoDivider.UI;

public sealed record UiPalette
{
    public required Color Window { get; init; }
    public required Color Panel { get; init; }
    public required Color Secondary { get; init; }
    public required Color Input { get; init; }
    public required Color Text { get; init; }
    public required Color Muted { get; init; }
    public required Color Accent { get; init; }
    public required Color Border { get; init; }
    public required Color Button { get; init; }
    public required Color ButtonHover { get; init; }
    public required Color Selection { get; init; }
    public required Color Danger { get; init; }
}

public static class Theme
{
    public static UiPalette Dark { get; } = new()
    {
        Window = Color.FromArgb(20, 22, 27),
        Panel = Color.FromArgb(30, 32, 39),
        Secondary = Color.FromArgb(40, 43, 52),
        Input = Color.FromArgb(36, 39, 47),
        Text = Color.FromArgb(230, 232, 237),
        Muted = Color.FromArgb(154, 160, 170),
        Accent = Color.FromArgb(76, 194, 255),
        Border = Color.FromArgb(58, 63, 75),
        Button = Color.FromArgb(47, 51, 62),
        ButtonHover = Color.FromArgb(58, 64, 78),
        Selection = Color.FromArgb(35, 67, 92),
        Danger = Color.FromArgb(255, 105, 97)
    };

    public static UiPalette Light { get; } = new()
    {
        Window = Color.FromArgb(246, 246, 246),
        Panel = Color.White,
        Secondary = Color.FromArgb(237, 239, 242),
        Input = Color.White,
        Text = Color.FromArgb(32, 36, 43),
        Muted = Color.FromArgb(104, 112, 125),
        Accent = Color.FromArgb(0, 120, 212),
        Border = Color.FromArgb(213, 217, 224),
        Button = Color.FromArgb(233, 236, 241),
        ButtonHover = Color.FromArgb(218, 224, 233),
        Selection = Color.FromArgb(218, 235, 249),
        Danger = Color.FromArgb(196, 43, 28)
    };

    public static UiPalette Current(bool dark) => dark ? Dark : Light;

    public static void Apply(Control root, bool dark)
    {
        var palette = Current(dark);
        ApplyControl(root, palette);
    }

    private static void ApplyControl(Control control, UiPalette palette)
    {
        switch (control)
        {
            case Form:
            case Panel:
                control.BackColor = palette.Panel;
                control.ForeColor = palette.Text;
                break;
            case TextBox:
            case RichTextBox:
            case ComboBox:
            case NumericUpDown:
            case ListView:
                control.BackColor = palette.Input;
                control.ForeColor = palette.Text;
                break;
            case Button button:
                button.BackColor = palette.Button;
                button.ForeColor = palette.Text;
                button.FlatStyle = FlatStyle.Flat;
                button.FlatAppearance.BorderColor = palette.Border;
                button.FlatAppearance.BorderSize = 1;
                button.FlatAppearance.MouseOverBackColor = palette.ButtonHover;
                button.UseVisualStyleBackColor = false;
                break;
            case CheckBox:
            case RadioButton:
            case Label:
                control.ForeColor = palette.Text;
                break;
            case ProgressBar:
                control.BackColor = palette.Secondary;
                break;
            case PictureBox:
                control.BackColor = palette.Window;
                break;
            case ToolStrip toolStrip:
                toolStrip.BackColor = palette.Secondary;
                toolStrip.ForeColor = palette.Text;
                break;
        }

        foreach (Control child in control.Controls)
        {
            ApplyControl(child, palette);
        }
    }

    public static void StylePrimary(Button button, bool dark)
    {
        var palette = Current(dark);
        button.BackColor = palette.Accent;
        button.ForeColor = dark ? Color.FromArgb(12, 18, 24) : Color.White;
        button.FlatAppearance.BorderColor = palette.Accent;
        button.FlatAppearance.MouseOverBackColor = dark
            ? Color.FromArgb(126, 214, 255)
            : Color.FromArgb(0, 103, 183);
    }

    public static void StyleDanger(Button button, bool dark)
    {
        var palette = Current(dark);
        button.ForeColor = palette.Danger;
    }

    public static void RoundPanel(Panel panel, Color color, int radius = 6)
    {
        panel.Paint += (_, e) =>
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using var brush = new SolidBrush(color);
            using var path = RoundedPath(panel.ClientRectangle, radius);
            e.Graphics.FillPath(brush, path);
        };
    }

    private static GraphicsPath RoundedPath(Rectangle bounds, int radius)
    {
        var path = new GraphicsPath();
        var diameter = radius * 2;
        path.AddArc(bounds.X, bounds.Y, diameter, diameter, 180, 90);
        path.AddArc(bounds.Right - diameter, bounds.Y, diameter, diameter, 270, 90);
        path.AddArc(bounds.Right - diameter, bounds.Bottom - diameter, diameter, diameter, 0, 90);
        path.AddArc(bounds.X, bounds.Bottom - diameter, diameter, diameter, 90, 90);
        path.CloseFigure();
        return path;
    }
}
