using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using SlideVideoDivider.Core;

namespace SlideVideoDivider.UI;

public sealed class MainForm : Form
{
    private readonly AppSettings _settings = AppSettings.Load();
    private readonly EditHistory _editHistory = new();
    private readonly ToolTip _toolTip = new();
    private readonly string? _initialProjectPath;
    private readonly ImageList _slideImageList = new()
    {
        ImageSize = new Size(150, 84),
        ColorDepth = ColorDepth.Depth32Bit
    };

    private SlideProject? _project;
    private CancellationTokenSource? _cancellation;
    private bool _busy;
    private bool _dirty;
    private bool _loadingSlide;
    private bool _refreshingList;
    private int _selectedSlideIndex = -1;

    private Panel _headerPanel = null!;
    private Panel _convertPage = null!;
    private Panel _notesPage = null!;
    private Panel _settingsPage = null!;
    private Button _convertTab = null!;
    private Button _notesTab = null!;
    private Button _settingsTab = null!;
    private StatusStrip _statusStrip = null!;
    private ToolStripStatusLabel _statusLabel = null!;
    private ToolStripProgressBar _statusProgress = null!;
    private ToolStripStatusLabel _countLabel = null!;

    private TextBox _videoTextBox = null!;
    private TextBox _outputTextBox = null!;
    private ComboBox _modelComboBox = null!;
    private TextBox _modelPathTextBox = null!;
    private CheckBox _useGpuCheckBox = null!;
    private NumericUpDown _sceneThresholdNumeric = null!;
    private NumericUpDown _minimumSlideNumeric = null!;
    private TextBox _promptTextBox = null!;
    private Label _toolStatusLabel = null!;
    private Label _modelStatusLabel = null!;
    private Button _startButton = null!;
    private Button _cancelButton = null!;
    private Button _downloadModelButton = null!;
    private ProgressBar _convertProgress = null!;
    private Label _convertStatusLabel = null!;

    private CheckBox _darkModeCheckBox = null!;
    private TextBox _settingsOutputTextBox = null!;
    private TextBox _settingsModelDirectoryTextBox = null!;
    private NumericUpDown _pdfMinimumFontNumeric = null!;

    private TextBox _searchTextBox = null!;
    private ListView _slideListView = null!;
    private PictureBox _previewPictureBox = null!;
    private Label _previewCaptionLabel = null!;
    private TextBox _transcriptTextBox = null!;
    private TextBox _startTimeTextBox = null!;
    private TextBox _endTimeTextBox = null!;
    private Label _transcriptCaptionLabel = null!;
    private Button _openProjectButton = null!;
    private Button _saveButton = null!;
    private Button _undoButton = null!;
    private Button _redoButton = null!;
    private Button _pdfButton = null!;
    private Button _splitButton = null!;
    private Button _deleteButton = null!;
    private Button _movePreviousButton = null!;
    private Button _moveNextButton = null!;
    private Button _replaceImageButton = null!;

    public MainForm(string? initialProjectPath = null)
    {
        _initialProjectPath = initialProjectPath;
        Text = "SlideVideoDivider";
        StartPosition = FormStartPosition.CenterScreen;
        ClientSize = new Size(1180, 760);
        MinimumSize = new Size(980, 660);
        AutoScaleMode = AutoScaleMode.Dpi;
        KeyPreview = true;
        Font = new Font("Yu Gothic", 9F, FontStyle.Regular, GraphicsUnit.Point);
        Icon = SystemIcons.Application;

        BuildHeader();
        BuildStatusStrip();
        BuildConvertPage();
        BuildSettingsPage();
        BuildNotesPage();
        _editHistory.Changed += (_, _) => UpdateHistoryUi();

        Controls.Add(_convertPage);
        Controls.Add(_settingsPage);
        Controls.Add(_notesPage);
        Controls.Add(_statusStrip);
        Controls.Add(_headerPanel);

        AllowDrop = true;
        DragEnter += MainForm_DragEnter;
        DragDrop += MainForm_DragDrop;

        Shown += (_, _) =>
        {
            ApplyTheme();
            UpdateToolStatus();
            if (!string.IsNullOrWhiteSpace(_initialProjectPath) && File.Exists(_initialProjectPath))
            {
                try
                {
                    LoadProjectIntoUi(ProjectStore.Load(_initialProjectPath));
                    ShowPage(1);
                }
                catch (Exception ex)
                {
                    MessageBox.Show(this, ex.Message, "プロジェクトを開けません", MessageBoxButtons.OK, MessageBoxIcon.Error);
                }
            }
        };
        FormClosing += MainForm_FormClosing;
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        UpdateTitleBarTheme();
    }

    private void BuildHeader()
    {
        _headerPanel = new Panel
        {
            Dock = DockStyle.Top,
            Height = 48,
            Padding = new Padding(14, 7, 14, 7)
        };

        var title = new Label
        {
            Text = "SlideVideoDivider",
            AutoSize = true,
            Location = new Point(4, 12),
            Font = new Font("Yu Gothic", 12F, FontStyle.Bold, GraphicsUnit.Point)
        };
        _headerPanel.Controls.Add(title);

        var navigation = new FlowLayoutPanel
        {
            AutoSize = true,
            Location = new Point(190, 7),
            FlowDirection = FlowDirection.LeftToRight,
            WrapContents = false,
            Margin = new Padding(0),
            Padding = new Padding(0)
        };
        _convertTab = CreateNavButton("変換");
        _notesTab = CreateNavButton("スライドノート");
        _settingsTab = CreateNavButton("設定");
        _convertTab.Click += (_, _) => ShowPage(0);
        _notesTab.Click += (_, _) => ShowPage(1);
        _settingsTab.Click += (_, _) => ShowPage(2);
        navigation.Controls.Add(_convertTab);
        navigation.Controls.Add(_notesTab);
        navigation.Controls.Add(_settingsTab);
        _headerPanel.Controls.Add(navigation);
    }

    private static Button CreateNavButton(string text)
    {
        return new Button
        {
            Text = text,
            AutoSize = false,
            Width = text == "スライドノート" ? 130 : 86,
            Height = 32,
            Margin = new Padding(0, 0, 4, 0),
            UseVisualStyleBackColor = false
        };
    }

    private void BuildStatusStrip()
    {
        _statusStrip = new StatusStrip { ShowItemToolTips = true };
        _statusLabel = new ToolStripStatusLabel("準備できました") { Spring = true, TextAlign = ContentAlignment.MiddleLeft };
        _statusProgress = new ToolStripProgressBar { Width = 170, Visible = false };
        _countLabel = new ToolStripStatusLabel("スライド: 0");
        _statusStrip.Items.Add(_statusLabel);
        _statusStrip.Items.Add(_statusProgress);
        _statusStrip.Items.Add(_countLabel);
    }

    private void BuildConvertPage()
    {
        _convertPage = new Panel { Dock = DockStyle.Fill, AutoScroll = true, Padding = new Padding(18) };
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            ColumnCount = 3,
            Padding = new Padding(0, 0, 8, 18)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 120));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 132));
        _convertPage.Controls.Add(layout);

        var row = 0;
        AddTitleRow(layout, ref row, "音声付きスライド動画をノートPDFへ変換");
        AddSectionRow(layout, ref row, "入力・出力");
        AddFieldRow(layout, ref row, "動画ファイル", _videoTextBox = new TextBox(), "参照...", out var videoBrowse);
        videoBrowse!.Click += (_, _) => BrowseVideo();
        AddFieldRow(layout, ref row, "出力フォルダ", _outputTextBox = new TextBox { Text = _settings.OutputDirectory }, "参照...", out var outputBrowse);
        outputBrowse!.Click += (_, _) => BrowseOutputFolder(_outputTextBox);
        AddInfoRow(layout, ref row, "MP4 / MOV / MKV / WebM に対応。動画をドラッグ＆ドロップしても選択できます。");

        AddSectionRow(layout, ref row, "文字起こし・スライド検出");
        _modelComboBox = new ComboBox
        {
            DropDownStyle = ComboBoxStyle.DropDownList
        };
        _modelComboBox.Items.AddRange(ModelCatalog.Choices.ToArray());
        _modelComboBox.SelectedIndex = Math.Clamp(_settings.ModelIndex, 0, ModelCatalog.Choices.Count - 1);
        _modelComboBox.SelectedIndexChanged += (_, _) => UpdateDefaultModelPath();
        AddFieldRow(layout, ref row, "認識モデル", _modelComboBox, null);
        AddFieldRow(layout, ref row, "モデルファイル", _modelPathTextBox = new TextBox(), "参照...", out var modelBrowse);
        modelBrowse!.Click += (_, _) => BrowseModelFile();
        _downloadModelButton = new Button { Text = "ダウンロード", Width = 120 };
        _downloadModelButton.Click += async (_, _) => await DownloadModelAsync();
        AddActionRow(layout, ref row, [_downloadModelButton, new Label { Text = "初回のみ必要です。ローカルで実行されます。", AutoSize = true, Margin = new Padding(8, 8, 0, 0) }]);
        _modelStatusLabel = new Label { AutoSize = true, ForeColor = Color.Gray, Margin = new Padding(4, 2, 4, 8) };
        AddSpanRow(layout, ref row, _modelStatusLabel);

        _useGpuCheckBox = new CheckBox { Text = "NVIDIA GPU（CUDA）を使用", Checked = _settings.UseGpu, AutoSize = true };
        AddFieldRow(layout, ref row, "実行デバイス", _useGpuCheckBox, null);
        _sceneThresholdNumeric = new NumericUpDown
        {
            Minimum = 0.05m,
            Maximum = 0.50m,
            Increment = 0.01m,
            DecimalPlaces = 2,
            Value = (decimal)_settings.SceneThreshold
        };
        AddFieldRow(layout, ref row, "切替検出しきい値", _sceneThresholdNumeric, null);
        _minimumSlideNumeric = new NumericUpDown
        {
            Minimum = 0.5m,
            Maximum = 10m,
            Increment = 0.5m,
            DecimalPlaces = 1,
            Value = (decimal)_settings.MinimumSlideSeconds
        };
        AddFieldRow(layout, ref row, "最短表示時間", _minimumSlideNumeric, null);
        _promptTextBox = new TextBox
        {
            Text = _settings.InitialPrompt,
            Multiline = true,
            Height = 66,
            AcceptsReturn = true,
            ScrollBars = ScrollBars.Vertical
        };
        AddFieldRow(layout, ref row, "専門用語ヒント", _promptTextBox, null);

        AddSectionRow(layout, ref row, "実行");
        _startButton = new Button { Text = "変換開始", Width = 132, Height = 36 };
        Theme.StylePrimary(_startButton, _settings.DarkMode);
        _startButton.Click += async (_, _) => await StartConversionAsync();
        _cancelButton = new Button { Text = "キャンセル", Width = 112, Height = 36, Enabled = false };
        _cancelButton.Click += (_, _) => _cancellation?.Cancel();
        AddActionRow(layout, ref row, [_startButton, _cancelButton]);
        _convertProgress = new ProgressBar { Dock = DockStyle.Fill, Height = 22 };
        AddSpanRow(layout, ref row, _convertProgress);
        _convertStatusLabel = new Label { AutoSize = true, Margin = new Padding(4, 6, 4, 4) };
        AddSpanRow(layout, ref row, _convertStatusLabel);
        _toolStatusLabel = new Label { AutoSize = true, Margin = new Padding(4, 6, 4, 4) };
        AddSpanRow(layout, ref row, _toolStatusLabel);

        _outputTextBox.Text = _settings.OutputDirectory;
        UpdateDefaultModelPath();
        UpdateModelStatus();
    }

    private void BuildSettingsPage()
    {
        _settingsPage = new Panel { Dock = DockStyle.Fill, AutoScroll = true, Padding = new Padding(18) };
        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Top,
            AutoSize = true,
            ColumnCount = 3,
            Padding = new Padding(0, 0, 8, 18)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 150));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 132));
        _settingsPage.Controls.Add(layout);

        var row = 0;
        AddTitleRow(layout, ref row, "設定");
        AddSectionRow(layout, ref row, "外観");
        _darkModeCheckBox = new CheckBox { Text = "ダークモード", Checked = _settings.DarkMode, AutoSize = true };
        _darkModeCheckBox.CheckedChanged += (_, _) =>
        {
            _settings.DarkMode = _darkModeCheckBox.Checked;
            ApplyTheme();
        };
        AddFieldRow(layout, ref row, "テーマ", _darkModeCheckBox, null);

        AddSectionRow(layout, ref row, "フォルダ");
        AddFieldRow(layout, ref row, "既定の出力先", _settingsOutputTextBox = new TextBox { Text = _settings.OutputDirectory }, "参照...", out var settingsOutputBrowse);
        settingsOutputBrowse!.Click += (_, _) => BrowseOutputFolder(_settingsOutputTextBox);
        AddFieldRow(layout, ref row, "モデル保存先", _settingsModelDirectoryTextBox = new TextBox { Text = _settings.ModelDirectory }, "参照...", out var settingsModelBrowse);
        settingsModelBrowse!.Click += (_, _) => BrowseOutputFolder(_settingsModelDirectoryTextBox);
        _settingsOutputTextBox.Leave += (_, _) =>
        {
            _settings.OutputDirectory = _settingsOutputTextBox.Text.Trim();
            _outputTextBox.Text = _settings.OutputDirectory;
        };
        _settingsModelDirectoryTextBox.Leave += (_, _) =>
        {
            _settings.ModelDirectory = _settingsModelDirectoryTextBox.Text.Trim();
            UpdateDefaultModelPath();
        };

        AddSectionRow(layout, ref row, "PDF");
        _pdfMinimumFontNumeric = new NumericUpDown
        {
            Minimum = 5.0m,
            Maximum = 10.0m,
            Increment = 0.25m,
            DecimalPlaces = 2,
            Value = (decimal)_settings.PdfMinimumFontSize
        };
        AddFieldRow(layout, ref row, "最小フォントサイズ", _pdfMinimumFontNumeric, null);
        AddInfoRow(layout, ref row, "単位はptです。小さい値ほど多くの文字を1ページに収められます。");

        AddSectionRow(layout, ref row, "このアプリについて");
        AddInfoRow(layout, ref row, "SlideVideoDivider 1.1.0");
        AddInfoRow(layout, ref row, "処理はすべてローカルで実行されます。音声や動画は外部サービスへ送信されません。");
    }

    private void BuildNotesPage()
    {
        _notesPage = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8) };

        var toolbar = new Panel { Dock = DockStyle.Top, Height = 82, Padding = new Padding(0, 6, 0, 6) };
        var toolbarLayout = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            WrapContents = true,
            AutoSize = false,
            Margin = new Padding(0),
            Padding = new Padding(0)
        };
        _searchTextBox = new TextBox { Width = 150, PlaceholderText = "スライドを検索" };
        _searchTextBox.TextChanged += (_, _) => RefreshSlideList();
        toolbarLayout.Controls.Add(_searchTextBox);
        toolbarLayout.Controls.Add(CreateToolbarSeparator());
        _openProjectButton = CreateToolbarButton("開く");
        _openProjectButton.Click += (_, _) => OpenProject();
        _saveButton = CreateToolbarButton("保存");
        _saveButton.Click += (_, _) => SaveProject();
        _undoButton = CreateToolbarButton("元に戻す");
        _undoButton.Click += (_, _) => UndoEdit();
        _redoButton = CreateToolbarButton("やり直し");
        _redoButton.Click += (_, _) => RedoEdit();
        _pdfButton = CreateToolbarButton("PDF出力");
        Theme.StylePrimary(_pdfButton, _settings.DarkMode);
        _pdfButton.Click += async (_, _) => await ExportPdfAsync();
        _splitButton = CreateToolbarButton("中点で分割");
        _splitButton.Click += async (_, _) => await SplitSelectedSlideAsync();
        _deleteButton = CreateToolbarButton("削除");
        Theme.StyleDanger(_deleteButton, _settings.DarkMode);
        _deleteButton.Click += (_, _) => DeleteSelectedSlide();
        _movePreviousButton = CreateToolbarButton("文字を前へ");
        _movePreviousButton.Click += (_, _) => MoveTranscript(-1);
        _moveNextButton = CreateToolbarButton("文字を次へ");
        _moveNextButton.Click += (_, _) => MoveTranscript(1);
        _replaceImageButton = CreateToolbarButton("画像差替");
        _replaceImageButton.Click += (_, _) => ReplaceSelectedImage();
        toolbarLayout.Controls.AddRange(new Control[]
        {
            _openProjectButton, _saveButton, _undoButton, _redoButton, _pdfButton, CreateToolbarSeparator(),
            _splitButton, _deleteButton, CreateToolbarSeparator(), _movePreviousButton, _moveNextButton, _replaceImageButton
        });
        toolbar.Controls.Add(toolbarLayout);

        var mainSplit = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Size = new Size(1000, 600),
            Orientation = Orientation.Vertical,
            SplitterWidth = 6,
            Panel1MinSize = 190,
            Panel2MinSize = 620,
            SplitterDistance = 230
        };

        _slideListView = new ListView
        {
            Dock = DockStyle.Fill,
            View = View.LargeIcon,
            MultiSelect = false,
            HideSelection = false,
            LargeImageList = _slideImageList,
            BorderStyle = BorderStyle.FixedSingle,
            BackColor = Color.FromArgb(36, 39, 47),
            ForeColor = Color.White,
            Font = new Font("Yu Gothic", 8.5F, FontStyle.Regular, GraphicsUnit.Point)
        };
        _slideListView.ItemSelectionChanged += (_, _) => LoadSelectedSlide();
        mainSplit.Panel1.Controls.Add(_slideListView);

        var previewSplit = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Size = new Size(760, 600),
            Orientation = Orientation.Vertical,
            SplitterWidth = 6,
            Panel1MinSize = 250,
            Panel2MinSize = 280,
            SplitterDistance = 410
        };

        _previewCaptionLabel = new Label
        {
            Dock = DockStyle.Top,
            Height = 26,
            Text = "スライドプレビュー",
            Padding = new Padding(4, 5, 4, 0)
        };
        _previewPictureBox = new PictureBox
        {
            Dock = DockStyle.Fill,
            SizeMode = PictureBoxSizeMode.Zoom,
            BorderStyle = BorderStyle.FixedSingle
        };
        previewSplit.Panel1.Controls.Add(_previewPictureBox);
        previewSplit.Panel1.Controls.Add(_previewCaptionLabel);
        _previewCaptionLabel.SendToBack();

        var transcriptHeader = new Panel
        {
            Dock = DockStyle.Top,
            Height = 64,
            Padding = new Padding(4, 2, 4, 2)
        };
        transcriptHeader.Controls.Add(new Label
        {
            Text = "開始（秒）",
            AutoSize = true,
            Location = new Point(4, 7)
        });
        _startTimeTextBox = new TextBox { Location = new Point(72, 4), Width = 90 };
        _startTimeTextBox.Leave += TimeBox_Leave;
        transcriptHeader.Controls.Add(_startTimeTextBox);
        transcriptHeader.Controls.Add(new Label
        {
            Text = "終了（秒）",
            AutoSize = true,
            Location = new Point(174, 7)
        });
        _endTimeTextBox = new TextBox { Location = new Point(242, 4), Width = 90 };
        _endTimeTextBox.Leave += TimeBox_Leave;
        transcriptHeader.Controls.Add(_endTimeTextBox);
        _transcriptCaptionLabel = new Label
        {
            Text = "文字起こし",
            AutoSize = true,
            Location = new Point(4, 38)
        };
        transcriptHeader.Controls.Add(_transcriptCaptionLabel);

        _transcriptTextBox = new TextBox
        {
            Dock = DockStyle.Fill,
            Multiline = true,
            AcceptsReturn = true,
            AcceptsTab = true,
            ScrollBars = ScrollBars.Vertical,
            WordWrap = true,
            BorderStyle = BorderStyle.FixedSingle,
            Font = new Font("Yu Gothic", 10F, FontStyle.Regular, GraphicsUnit.Point)
        };
        _transcriptTextBox.TextChanged += (_, _) =>
        {
            if (_loadingSlide || _selectedSlideIndex < 0 || _project is null)
            {
                return;
            }
            _project.Slides[_selectedSlideIndex].Transcript = _transcriptTextBox.Text;
            _dirty = true;
            UpdateWindowTitle();
        };

        previewSplit.Panel2.Controls.Add(_transcriptTextBox);
        previewSplit.Panel2.Controls.Add(transcriptHeader);
        transcriptHeader.SendToBack();
        mainSplit.Panel2.Controls.Add(previewSplit);

        _notesPage.Controls.Add(mainSplit);
        _notesPage.Controls.Add(toolbar);
        toolbar.SendToBack();
        UpdateNotesButtons();
    }

    private static Button CreateToolbarButton(string text)
    {
        return new Button
        {
            Text = text,
            AutoSize = true,
            Height = 30,
            Margin = new Padding(0, 0, 5, 0),
            Padding = new Padding(6, 3, 6, 3),
            UseVisualStyleBackColor = false
        };
    }

    private static Label CreateToolbarSeparator()
    {
        return new Label { Text = "│", AutoSize = true, Margin = new Padding(4, 5, 4, 0) };
    }

    private static void AddTitleRow(TableLayoutPanel table, ref int row, string text)
    {
        var label = new Label
        {
            Text = text,
            AutoSize = true,
            Font = new Font("Yu Gothic", 15F, FontStyle.Bold, GraphicsUnit.Point),
            Margin = new Padding(0, 0, 0, 14)
        };
        table.Controls.Add(label, 0, row);
        table.SetColumnSpan(label, 3);
        row++;
    }

    private static void AddSectionRow(TableLayoutPanel table, ref int row, string text)
    {
        var label = new Label
        {
            Text = text,
            AutoSize = true,
            Font = new Font("Yu Gothic", 10F, FontStyle.Bold, GraphicsUnit.Point),
            Margin = new Padding(0, 14, 0, 7)
        };
        table.Controls.Add(label, 0, row);
        table.SetColumnSpan(label, 3);
        row++;
    }

    private static void AddFieldRow(
        TableLayoutPanel table,
        ref int row,
        string labelText,
        Control value,
        string? buttonText)
    {
        AddFieldRow(table, ref row, labelText, value, buttonText, out _);
    }

    private static void AddFieldRow(
        TableLayoutPanel table,
        ref int row,
        string labelText,
        Control value,
        string? buttonText,
        out Button? button)
    {
        var label = new Label { Text = labelText, AutoSize = true, Margin = new Padding(0, 8, 8, 6) };
        table.Controls.Add(label, 0, row);
        value.Dock = DockStyle.Fill;
        value.Margin = new Padding(0, 4, 8, 4);
        table.Controls.Add(value, 1, row);
        button = null;
        if (buttonText is not null)
        {
            button = new Button { Text = buttonText, Dock = DockStyle.Fill, Height = 28, Margin = new Padding(0, 4, 0, 4) };
            table.Controls.Add(button, 2, row);
        }
        row++;
    }

    private static void AddSpanRow(TableLayoutPanel table, ref int row, Control control)
    {
        control.Dock = DockStyle.Fill;
        table.Controls.Add(control, 0, row);
        table.SetColumnSpan(control, 3);
        row++;
    }

    private static void AddInfoRow(TableLayoutPanel table, ref int row, string text)
    {
        AddSpanRow(table, ref row, new Label { Text = text, AutoSize = true, Margin = new Padding(0, 2, 0, 8) });
    }

    private static void AddActionRow(TableLayoutPanel table, ref int row, Control[] controls)
    {
        var panel = new FlowLayoutPanel
        {
            AutoSize = true,
            WrapContents = true,
            FlowDirection = FlowDirection.LeftToRight,
            Margin = new Padding(0, 4, 0, 8),
            Padding = new Padding(0)
        };
        panel.Controls.AddRange(controls);
        AddSpanRow(table, ref row, panel);
    }

    private void ShowPage(int index)
    {
        _convertPage.Visible = index == 0;
        _notesPage.Visible = index == 1;
        _settingsPage.Visible = index == 2;
        StyleNavButton(_convertTab, index == 0);
        StyleNavButton(_notesTab, index == 1);
        StyleNavButton(_settingsTab, index == 2);
    }

    private void StyleNavButton(Button button, bool active)
    {
        var palette = Theme.Current(_settings.DarkMode);
        button.BackColor = active ? palette.Accent : palette.Button;
        button.ForeColor = active
            ? (_settings.DarkMode ? Color.FromArgb(12, 18, 24) : Color.White)
            : palette.Text;
    }

    private void ApplyTheme()
    {
        Theme.Apply(this, _settings.DarkMode);
        StyleNavButton(_convertTab, _convertPage.Visible);
        StyleNavButton(_notesTab, _notesPage.Visible);
        StyleNavButton(_settingsTab, _settingsPage.Visible);
        Theme.StylePrimary(_startButton, _settings.DarkMode);
        Theme.StylePrimary(_pdfButton, _settings.DarkMode);
        Theme.StyleDanger(_deleteButton, _settings.DarkMode);
        UpdateTitleBarTheme();
        _slideListView.BackColor = Theme.Current(_settings.DarkMode).Input;
        _slideListView.ForeColor = Theme.Current(_settings.DarkMode).Text;
        _previewPictureBox.BackColor = Theme.Current(_settings.DarkMode).Window;
        UpdateWindowTitle();
    }

    private void UpdateWindowTitle()
    {
        Text = _project is null
            ? "SlideVideoDivider"
            : $"SlideVideoDivider — {Path.GetFileNameWithoutExtension(_project.VideoName)}{(_dirty ? " *" : string.Empty)}";
    }

    private void UpdateToolStatus()
    {
        var ffmpeg = ExternalTools.FindFfmpeg();
        var ffprobe = ExternalTools.FindFfprobe();
        _toolStatusLabel.Text = ffmpeg is null
            ? "FFmpeg: 見つかりません（PATHを確認してください）"
            : $"FFmpeg: {ffmpeg}    FFprobe: {(ffprobe is null ? "なし" : "あり")}";
    }

    private void UpdateDefaultModelPath()
    {
        if (_modelComboBox.SelectedItem is not ModelChoice choice)
        {
            return;
        }
        _modelPathTextBox.Text = Path.Combine(_settings.ModelDirectory, choice.FileName);
        UpdateModelStatus();
    }

    private void UpdateModelStatus()
    {
        if (_modelComboBox.SelectedItem is not ModelChoice choice)
        {
            return;
        }
        var path = _modelPathTextBox.Text.Trim();
        _modelStatusLabel.Text = File.Exists(path)
            ? $"モデル済み: {path}（{new FileInfo(path).Length / 1024.0 / 1024.0:F0} MB）"
            : $"{choice.Description}　※初回はダウンロードが必要です";
    }

    private async Task StartConversionAsync()
    {
        if (_busy)
        {
            return;
        }

        var videoPath = _videoTextBox.Text.Trim();
        if (!File.Exists(videoPath))
        {
            MessageBox.Show(this, "動画ファイルを選択してください。", "入力エラー", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }

        var ffmpeg = ExternalTools.FindFfmpeg();
        if (ffmpeg is null)
        {
            MessageBox.Show(this, "FFmpegが見つかりません。FFmpegをインストールし、PATHに追加してください。", "FFmpegエラー",
                MessageBoxButtons.OK, MessageBoxIcon.Error);
            return;
        }

        if (_modelComboBox.SelectedItem is not ModelChoice model)
        {
            return;
        }

        var outputRoot = _outputTextBox.Text.Trim();
        if (string.IsNullOrWhiteSpace(outputRoot))
        {
            outputRoot = _settings.OutputDirectory;
            _outputTextBox.Text = outputRoot;
        }

        var modelPath = _modelPathTextBox.Text.Trim();
        if (string.IsNullOrWhiteSpace(modelPath))
        {
            modelPath = Path.Combine(_settings.ModelDirectory, model.FileName);
            _modelPathTextBox.Text = modelPath;
        }

        var outputDirectory = Path.Combine(outputRoot, MakeSafeFolderName(Path.GetFileNameWithoutExtension(videoPath)));
        var slidesDirectory = Path.Combine(outputDirectory, "slides");
        var wavPath = Path.Combine(outputDirectory, "_audio_16k.wav");

        _cancellation = new CancellationTokenSource();
        var token = _cancellation.Token;
        SetBusy(true);
        _convertProgress.Value = 0;
        _statusProgress.Visible = true;
        IProgress<string> statusProgress = new Progress<string>(message =>
        {
            _convertStatusLabel.Text = message;
            _statusLabel.Text = message;
        });
        IProgress<double> overallProgress = new Progress<double>(value =>
        {
            var clamped = Math.Clamp(value, 0, 100);
            _convertProgress.Value = (int)clamped;
            _statusProgress.Value = (int)clamped;
        });

        try
        {
            Directory.CreateDirectory(slidesDirectory);
            overallProgress.Report(2);
            statusProgress.Report("動画情報を確認しています...");
            var duration = await ExternalTools.ProbeDurationAsync(videoPath, token);

            var sceneProgress = new Progress<double>(value => overallProgress.Report(5 + value * 0.20));
            var boundaries = await ExternalTools.DetectSceneTimesAsync(
                ffmpeg, videoPath, duration,
                (double)_sceneThresholdNumeric.Value,
                (double)_minimumSlideNumeric.Value,
                statusProgress, sceneProgress, token);

            var slideProgress = new Progress<double>(value => overallProgress.Report(25 + value * 0.15));
            var slides = await ExternalTools.ExtractSlidesAsync(
                ffmpeg, videoPath, boundaries, duration, slidesDirectory,
                statusProgress, slideProgress, token);

            overallProgress.Report(41);
            await ExternalTools.ExtractAudioAsync(ffmpeg, videoPath, wavPath, statusProgress, token);
            overallProgress.Report(45);

            var downloadProgress = new Progress<long>(bytes => statusProgress.Report(
                $"モデル準備中... {TranscriptionService.FormatBytes(bytes)}"));
            await TranscriptionService.EnsureModelAsync(model, modelPath, statusProgress, downloadProgress, token);
            overallProgress.Report(50);

            var segments = await TranscriptionService.TranscribeAsync(
                wavPath, modelPath, _useGpuCheckBox.Checked, _promptTextBox.Text,
                50, 95, statusProgress, overallProgress, token);

            overallProgress.Report(96);
            statusProgress.Report("文字起こしをスライドへ対応付けています...");
            var project = new SlideProject
            {
                VideoPath = videoPath,
                VideoName = Path.GetFileName(videoPath),
                ModelName = model.DisplayName,
                ModelPath = modelPath,
                Slides = slides,
                Segments = segments
            };
            TranscriptAligner.AssignTranscripts(project);
            var projectPath = ProjectStore.GetDefaultProjectPath(outputDirectory, videoPath);
            ProjectStore.Save(projectPath, project);

            overallProgress.Report(100);
            statusProgress.Report($"完了: {slides.Count}枚のスライド / {segments.Count}発話");
            LoadProjectIntoUi(project);
            ShowPage(1);
            UpdateModelStatus();
            SaveSettingsFromUi();
        }
        catch (OperationCanceledException)
        {
            _convertProgress.Value = 0;
            statusProgress.Report("処理をキャンセルしました。");
        }
        catch (Exception ex)
        {
            statusProgress.Report("エラーが発生しました。");
            MessageBox.Show(this, ex.Message, "変換エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally
        {
            try
            {
                if (File.Exists(wavPath))
                {
                    File.Delete(wavPath);
                }
            }
            catch
            {
                // Ignore temporary audio cleanup failures.
            }
            _cancellation?.Dispose();
            _cancellation = null;
            SetBusy(false);
            _statusProgress.Visible = false;
        }
    }

    private async Task DownloadModelAsync()
    {
        if (_busy || _modelComboBox.SelectedItem is not ModelChoice model)
        {
            return;
        }

        var path = _modelPathTextBox.Text.Trim();
        if (string.IsNullOrWhiteSpace(path))
        {
            path = Path.Combine(_settings.ModelDirectory, model.FileName);
            _modelPathTextBox.Text = path;
        }

        SetBusy(true);
        _statusProgress.Visible = true;
        _statusProgress.Style = ProgressBarStyle.Marquee;
        var status = new Progress<string>(message =>
        {
            _modelStatusLabel.Text = message;
            _statusLabel.Text = message;
        });
        var bytes = new Progress<long>(value => _modelStatusLabel.Text =
            $"ダウンロード中... {TranscriptionService.FormatBytes(value)}");

        try
        {
            await TranscriptionService.EnsureModelAsync(model, path, status, bytes);
            UpdateModelStatus();
            _statusLabel.Text = "モデルのダウンロードが完了しました。";
        }
        catch (Exception ex)
        {
            _modelStatusLabel.Text = "モデルのダウンロードに失敗しました。";
            MessageBox.Show(this, ex.Message, "モデルダウンロードエラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally
        {
            _statusProgress.Style = ProgressBarStyle.Blocks;
            _statusProgress.Visible = false;
            SetBusy(false);
        }
    }

    private void SetBusy(bool busy)
    {
        _busy = busy;
        _startButton.Enabled = !busy;
        _downloadModelButton.Enabled = !busy;
        _cancelButton.Enabled = busy;
        UseWaitCursor = busy;
        UpdateNotesButtons();
    }

    private void LoadProjectIntoUi(SlideProject project)
    {
        CommitSelectedTranscript();
        _project = project;
        _editHistory.Clear();
        _dirty = false;
        _selectedSlideIndex = -1;
        RefreshSlideList();
        if (_slideListView.Items.Count > 0)
        {
            _slideListView.Items[0].Selected = true;
        }
        UpdateWindowTitle();
        UpdateNotesButtons();
        _countLabel.Text = $"スライド: {project.Slides.Count}";
        if (!string.IsNullOrWhiteSpace(project.VideoPath))
        {
            _videoTextBox.Text = project.VideoPath;
        }
    }

    private void RefreshSlideList()
    {
        if (_project is null || _refreshingList)
        {
            return;
        }

        _refreshingList = true;
        var previousIndex = _selectedSlideIndex;
        _slideImageList.Images.Clear();
        _slideListView.Items.Clear();
        _slideListView.BeginUpdate();
        var query = _searchTextBox.Text.Trim();
        ListViewItem? selectedItem = null;

        for (var i = 0; i < _project.Slides.Count; i++)
        {
            var slide = _project.Slides[i];
            if (!string.IsNullOrEmpty(query) &&
                !slide.Number.ToString().Contains(query, StringComparison.OrdinalIgnoreCase) &&
                !slide.Transcript.Contains(query, StringComparison.OrdinalIgnoreCase) &&
                !FormatSeconds(slide.Start).Contains(query, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            var imageIndex = -1;
            try
            {
                if (File.Exists(slide.ImagePath))
                {
                    using var source = Image.FromFile(slide.ImagePath);
                    using var thumbnail = source.GetThumbnailImage(150, 84, null, IntPtr.Zero);
                    imageIndex = _slideImageList.Images.Count;
                    _slideImageList.Images.Add(thumbnail);
                }
            }
            catch
            {
                // Keep the slide in the list even if its image is unreadable.
            }

            var item = new ListViewItem($"{slide.Number:D2}  {FormatSeconds(slide.Start)}", imageIndex)
            {
                Tag = i,
                UseItemStyleForSubItems = false
            };
            _slideListView.Items.Add(item);
            if (i == previousIndex)
            {
                selectedItem = item;
            }
        }

        _slideListView.EndUpdate();
        _refreshingList = false;
        if (selectedItem is not null)
        {
            selectedItem.Selected = true;
        }
        else if (_slideListView.Items.Count > 0 && previousIndex >= 0)
        {
            var target = _slideListView.Items.Cast<ListViewItem>()
                .FirstOrDefault(item => (int)item.Tag! == previousIndex);
            if (target is not null)
            {
                target.Selected = true;
            }
        }
        UpdateNotesButtons();
    }

    private void LoadSelectedSlide()
    {
        if (_refreshingList || _project is null)
        {
            return;
        }

        CommitSelectedTranscript();
        if (_slideListView.SelectedItems.Count == 0)
        {
            _selectedSlideIndex = -1;
            ClearSlideEditor();
            UpdateNotesButtons();
            return;
        }

        _selectedSlideIndex = (int)_slideListView.SelectedItems[0].Tag!;
        var slide = _project.Slides[_selectedSlideIndex];
        _loadingSlide = true;
        try
        {
            var oldImage = _previewPictureBox.Image;
            _previewPictureBox.Image = null;
            oldImage?.Dispose();
            if (File.Exists(slide.ImagePath))
            {
                using var source = Image.FromFile(slide.ImagePath);
                _previewPictureBox.Image = new Bitmap(source);
            }

            _startTimeTextBox.Text = slide.Start.ToString("0.00", CultureInfo.InvariantCulture);
            _endTimeTextBox.Text = slide.End.ToString("0.00", CultureInfo.InvariantCulture);
            _transcriptTextBox.Text = slide.Transcript;
            _previewCaptionLabel.Text = $"スライド {slide.Number:D2} / {_project.Slides.Count}";
            _transcriptCaptionLabel.Text = $"文字起こし　{slide.Transcript.Length}文字";
        }
        finally
        {
            _loadingSlide = false;
        }
        UpdateNotesButtons();
    }

    private void ClearSlideEditor()
    {
        _previewPictureBox.Image?.Dispose();
        _previewPictureBox.Image = null;
        _startTimeTextBox.Clear();
        _endTimeTextBox.Clear();
        _transcriptTextBox.Clear();
        _previewCaptionLabel.Text = "スライドプレビュー";
        _transcriptCaptionLabel.Text = "文字起こし";
    }

    private void CommitSelectedTranscript()
    {
        if (_loadingSlide || _project is null || _selectedSlideIndex < 0 || _selectedSlideIndex >= _project.Slides.Count)
        {
            return;
        }
        _project.Slides[_selectedSlideIndex].Transcript = _transcriptTextBox.Text;
    }

    private void TimeBox_Leave(object? sender, EventArgs e)
    {
        if (_project is null || _selectedSlideIndex < 0)
        {
            return;
        }

        if (!double.TryParse(_startTimeTextBox.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var start) ||
            !double.TryParse(_endTimeTextBox.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var end) ||
            end <= start)
        {
            var slide = _project.Slides[_selectedSlideIndex];
            _startTimeTextBox.Text = slide.Start.ToString("0.00", CultureInfo.InvariantCulture);
            _endTimeTextBox.Text = slide.End.ToString("0.00", CultureInfo.InvariantCulture);
            return;
        }

        var index = _selectedSlideIndex;
        if (index > 0 && start < _project.Slides[index - 1].End)
        {
            start = _project.Slides[index - 1].End;
        }
        if (index + 1 < _project.Slides.Count && end > _project.Slides[index + 1].Start)
        {
            end = _project.Slides[index + 1].Start;
        }
        if (end <= start)
        {
            return;
        }

        _project.Slides[index].Start = start;
        _project.Slides[index].End = end;
        if (index > 0)
        {
            _project.Slides[index - 1].End = start;
        }
        if (index + 1 < _project.Slides.Count)
        {
            _project.Slides[index + 1].Start = end;
        }
        _startTimeTextBox.Text = start.ToString("0.00", CultureInfo.InvariantCulture);
        _endTimeTextBox.Text = end.ToString("0.00", CultureInfo.InvariantCulture);
        _dirty = true;
        RefreshSlideList();
        UpdateWindowTitle();
    }

    private void UpdateNotesButtons()
    {
        var hasSelection = _project is not null && _selectedSlideIndex >= 0;
        _openProjectButton.Enabled = !_busy;
        _saveButton.Enabled = _project is not null && !_busy;
        _undoButton.Enabled = _project is not null && _editHistory.CanUndo && !_busy;
        _redoButton.Enabled = _project is not null && _editHistory.CanRedo && !_busy;
        _pdfButton.Enabled = hasSelection && !_busy;
        _splitButton.Enabled = hasSelection && !_busy;
        _deleteButton.Enabled = hasSelection && (_project?.Slides.Count ?? 0) > 1 && !_busy;
        _movePreviousButton.Enabled = hasSelection && _selectedSlideIndex > 0 && !_busy;
        _moveNextButton.Enabled = hasSelection && _project is not null && _selectedSlideIndex < _project.Slides.Count - 1 && !_busy;
        _replaceImageButton.Enabled = hasSelection && !_busy;
    }

    private void UpdateHistoryUi()
    {
        if (_undoButton is null || _redoButton is null)
        {
            return;
        }
        _undoButton.Enabled = _project is not null && _editHistory.CanUndo && !_busy;
        _redoButton.Enabled = _project is not null && _editHistory.CanRedo && !_busy;
        _toolTip.SetToolTip(_undoButton, _editHistory.UndoDescription is null ? "元に戻せる操作がありません" : $"元に戻す: {_editHistory.UndoDescription}");
        _toolTip.SetToolTip(_redoButton, _editHistory.RedoDescription is null ? "やり直せる操作がありません" : $"やり直し: {_editHistory.RedoDescription}");
    }

    private void UndoEdit()
    {
        if (_busy || _project is null || !_editHistory.CanUndo)
        {
            return;
        }
        CommitSelectedTranscript();
        _editHistory.Undo();
        RefreshAfterHistoryEdit();
    }

    private void RedoEdit()
    {
        if (_busy || _project is null || !_editHistory.CanRedo)
        {
            return;
        }
        CommitSelectedTranscript();
        _editHistory.Redo();
        RefreshAfterHistoryEdit();
    }

    private void RefreshAfterHistoryEdit()
    {
        _dirty = true;
        if (_project is not null && _selectedSlideIndex >= 0 && _selectedSlideIndex < _project.Slides.Count)
        {
            _loadingSlide = true;
            _transcriptTextBox.Text = _project.Slides[_selectedSlideIndex].Transcript;
            _loadingSlide = false;
        }
        RefreshSlideList();
        if (_project is not null && _selectedSlideIndex >= 0 && _selectedSlideIndex < _project.Slides.Count)
        {
            var item = _slideListView.Items.Cast<ListViewItem>()
                .FirstOrDefault(x => (int)x.Tag! == _selectedSlideIndex);
            if (item is not null)
            {
                item.Selected = true;
                LoadSelectedSlide();
            }
        }
        UpdateWindowTitle();
        UpdateHistoryUi();
        _statusLabel.Text = _editHistory.UndoDescription is null
            ? "操作を元に戻しました。"
            : $"元に戻しました。次の操作: {_editHistory.UndoDescription}";
    }

    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        if (keyData == (Keys.Control | Keys.Z))
        {
            UndoEdit();
            return true;
        }
        if (keyData == (Keys.Control | Keys.Y) || keyData == (Keys.Control | Keys.Shift | Keys.Z))
        {
            RedoEdit();
            return true;
        }
        return base.ProcessCmdKey(ref msg, keyData);
    }

    private async Task SplitSelectedSlideAsync()
    {
        if (_project is null || _selectedSlideIndex < 0 || _busy)
        {
            return;
        }

        var slide = _project.Slides[_selectedSlideIndex];
        if (slide.Duration < 1)
        {
            MessageBox.Show(this, "分割するにはスライドの表示時間が1秒以上必要です。", "分割エラー",
                MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }

        var ffmpeg = ExternalTools.FindFfmpeg();
        if (ffmpeg is null)
        {
            return;
        }

        var splitTime = slide.Start + slide.Duration / 2;
        var imagePath = Path.Combine(_project.ProjectDirectory, "slides", $"slide_extra_{Guid.NewGuid():N}.jpg");
        Directory.CreateDirectory(Path.GetDirectoryName(imagePath)!);
        SetBusy(true);
        try
        {
            await ExternalTools.ExtractSingleFrameAsync(ffmpeg, _project.VideoPath, splitTime, imagePath);
            var oldEnd = slide.End;
            slide.End = splitTime;
            var newSlide = new SlideItem
            {
                Number = slide.Number + 1,
                Start = splitTime,
                End = oldEnd,
                ImagePath = imagePath,
                Transcript = string.Empty
            };
            _project.Slides.Insert(_selectedSlideIndex + 1, newSlide);
            _project.Renumber();
            _dirty = true;
            RefreshSlideList();
            var item = _slideListView.Items.Cast<ListViewItem>().FirstOrDefault(x => (int)x.Tag! == _selectedSlideIndex + 1);
            if (item is not null)
            {
                item.Selected = true;
            }
            _countLabel.Text = $"スライド: {_project.Slides.Count}";
            UpdateWindowTitle();
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "分割エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally
        {
            SetBusy(false);
            UpdateNotesButtons();
        }
    }

    private void DeleteSelectedSlide()
    {
        CommitSelectedTranscript();
        if (_project is null || _selectedSlideIndex < 0 || _project.Slides.Count <= 1)
        {
            return;
        }

        var slide = _project.Slides[_selectedSlideIndex];
        var result = MessageBox.Show(this, $"スライド {slide.Number:D2} を削除しますか？\nこのスライドの文字起こしも削除されます。",
            "スライド削除", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
        if (result != DialogResult.Yes)
        {
            return;
        }

        var index = _selectedSlideIndex;
        if (index > 0)
        {
            _project.Slides[index - 1].End = slide.End;
        }
        else if (_project.Slides.Count > 1)
        {
            _project.Slides[1].Start = slide.Start;
        }
        _project.Slides.RemoveAt(index);
        _project.Renumber();
        _selectedSlideIndex = Math.Min(index, _project.Slides.Count - 1);
        _dirty = true;
        RefreshSlideList();
        var item = _slideListView.Items.Cast<ListViewItem>().FirstOrDefault(x => (int)x.Tag! == _selectedSlideIndex);
        item?.Selected = true;
        _countLabel.Text = $"スライド: {_project.Slides.Count}";
        UpdateWindowTitle();
    }

    private void MoveTranscript(int direction)
    {
        CommitSelectedTranscript();
        if (_project is null || _selectedSlideIndex < 0)
        {
            return;
        }

        var targetIndex = _selectedSlideIndex + direction;
        if (targetIndex < 0 || targetIndex >= _project.Slides.Count)
        {
            return;
        }

        var source = _project.Slides[_selectedSlideIndex];
        var target = _project.Slides[targetIndex];
        var sourceText = TranscriptAligner.NormalizeTranscript(source.Transcript);
        var targetText = TranscriptAligner.NormalizeTranscript(target.Transcript);
        if (string.IsNullOrWhiteSpace(sourceText))
        {
            return;
        }

        void ApplyMove()
        {
            target.Transcript = string.IsNullOrWhiteSpace(targetText)
                ? sourceText
                : targetText + Environment.NewLine + sourceText;
            source.Transcript = string.Empty;
        }

        ApplyMove();
        _editHistory.Push(new EditAction
        {
            Description = direction < 0 ? "文字を前へ" : "文字を次へ",
            Undo = () =>
            {
                source.Transcript = sourceText;
                target.Transcript = targetText;
            },
            Redo = ApplyMove
        });
        _transcriptTextBox.Text = string.Empty;
        _dirty = true;
        RefreshSlideList();
        UpdateWindowTitle();
        UpdateHistoryUi();
        _statusLabel.Text = direction < 0 ? "文字起こしを前のスライドへ移動しました。" : "文字起こしを次のスライドへ移動しました。";
    }

    private void ReplaceSelectedImage()
    {
        if (_project is null || _selectedSlideIndex < 0)
        {
            return;
        }

        using var dialog = new OpenFileDialog
        {
            Filter = "画像ファイル|*.jpg;*.jpeg;*.png;*.bmp|すべてのファイル|*.*",
            Title = "差し替えるスライド画像を選択"
        };
        if (dialog.ShowDialog(this) != DialogResult.OK)
        {
            return;
        }

        ReplaceImageFile(dialog.FileName);
    }

    private void ReplaceImageFile(string sourcePath)
    {
        if (_project is null || _selectedSlideIndex < 0)
        {
            return;
        }

        try
        {
            var extension = Path.GetExtension(sourcePath).ToLowerInvariant();
            var destination = Path.Combine(_project.ProjectDirectory, "slides", $"slide_replace_{Guid.NewGuid():N}{extension}");
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
            File.Copy(sourcePath, destination);
            var slide = _project.Slides[_selectedSlideIndex];
            var previousImagePath = slide.ImagePath;
            slide.ImagePath = destination;
            _editHistory.Push(new EditAction
            {
                Description = "画像差し替え",
                Undo = () => slide.ImagePath = previousImagePath,
                Redo = () => slide.ImagePath = destination
            });
            _dirty = true;
            RefreshSlideList();
            var item = _slideListView.Items.Cast<ListViewItem>().FirstOrDefault(x => (int)x.Tag! == _selectedSlideIndex);
            if (item is not null)
            {
                item.Selected = true;
                LoadSelectedSlide();
            }
            UpdateWindowTitle();
            UpdateHistoryUi();
            _statusLabel.Text = "スライド画像を差し替えました。";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "画像差替エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private void OpenProject()
    {
        if (_busy)
        {
            return;
        }

        using var dialog = new OpenFileDialog
        {
            Filter = "プロジェクトファイル|*_project.json|JSONファイル|*.json|すべてのファイル|*.*",
            Title = "スライドノートプロジェクトを開く"
        };
        if (dialog.ShowDialog(this) != DialogResult.OK)
        {
            return;
        }

        try
        {
            var project = ProjectStore.Load(dialog.FileName);
            LoadProjectIntoUi(project);
            ShowPage(1);
            _statusLabel.Text = $"プロジェクトを開きました: {dialog.FileName}";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "プロジェクトを開けません", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private void OpenProjectFile(string path)
    {
        try
        {
            var project = ProjectStore.Load(path);
            LoadProjectIntoUi(project);
            ShowPage(1);
            _statusLabel.Text = $"プロジェクトを開きました: {path}";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "プロジェクトを開けません", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private void SaveProject()
    {
        if (_project is null)
        {
            return;
        }

        CommitSelectedTranscript();
        if (string.IsNullOrWhiteSpace(_project.ProjectPath))
        {
            using var dialog = new SaveFileDialog
            {
                Filter = "プロジェクトファイル|*_project.json|JSONファイル|*.json|すべてのファイル|*.*",
                FileName = Path.GetFileNameWithoutExtension(_project.VideoName) + "_project.json"
            };
            if (dialog.ShowDialog(this) != DialogResult.OK)
            {
                return;
            }
            _project.ProjectPath = dialog.FileName;
        }

        try
        {
            ProjectStore.Save(_project.ProjectPath, _project);
            _dirty = false;
            UpdateWindowTitle();
            _statusLabel.Text = $"プロジェクトを保存しました: {_project.ProjectPath}";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "保存エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private async Task ExportPdfAsync()
    {
        if (_project is null || _busy)
        {
            return;
        }

        CommitSelectedTranscript();
        _ = Enum.TryParse<PdfExportMode>(_settings.PdfExportMode, out var currentMode);
        using var optionsDialog = new PdfExportDialog(currentMode, _settings.PdfMinimumFontSize, _settings.DarkMode);
        if (optionsDialog.ShowDialog(this) != DialogResult.OK)
        {
            return;
        }

        var options = optionsDialog.Options;
        var suffix = options.Mode switch
        {
            PdfExportMode.SlidesOnly => "_スライド",
            PdfExportMode.TranscriptsOnly => "_文字起こし",
            _ => "_スライドノート"
        };
        using var dialog = new SaveFileDialog
        {
            Filter = "PDFファイル|*.pdf|すべてのファイル|*.*",
            FileName = Path.GetFileNameWithoutExtension(_project.VideoName) + suffix + ".pdf",
            Title = "PDFを保存"
        };
        if (dialog.ShowDialog(this) != DialogResult.OK)
        {
            return;
        }

        _settings.PdfExportMode = options.Mode.ToString();
        _settings.PdfMinimumFontSize = options.MinimumFontSize;
        if (_pdfMinimumFontNumeric.Value != (decimal)options.MinimumFontSize)
        {
            _pdfMinimumFontNumeric.Value = (decimal)options.MinimumFontSize;
        }

        SetBusy(true);
        _statusProgress.Visible = true;
        var status = new Progress<string>(message => _statusLabel.Text = message);
        var progress = new Progress<double>(value => _statusProgress.Value = (int)Math.Clamp(value, 0, 100));
        try
        {
            await PdfExporter.ExportAsync(_project, dialog.FileName, options, status, progress);
            _statusLabel.Text = $"PDFを書き出しました: {dialog.FileName}";
            var result = MessageBox.Show(this, "PDFを書き出しました。開きますか？", "完了", MessageBoxButtons.YesNo,
                MessageBoxIcon.Information);
            if (result == DialogResult.Yes)
            {
                Process.Start(new ProcessStartInfo(dialog.FileName) { UseShellExecute = true });
            }
        }
        catch (PdfLayoutException ex)
        {
            var numbers = string.Join("、", ex.SlideNumbers.Select(x => x.ToString("D2")));
            MessageBox.Show(this, $"スライド {numbers} の文字起こしが長すぎます。\n現在の最小フォントサイズは {ex.MinimumFontSize:0.00}pt です。\nPDF出力オプションで変更してください。",
                "PDFレイアウト警告", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            _statusLabel.Text = "文字量オーバーのためPDF出力を中止しました。";
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "PDF出力エラー", MessageBoxButtons.OK, MessageBoxIcon.Error);
            _statusLabel.Text = "PDF出力に失敗しました。";
        }
        finally
        {
            _statusProgress.Visible = false;
            SetBusy(false);
            UpdateNotesButtons();
        }
    }

    private void BrowseVideo()
    {
        using var dialog = new OpenFileDialog
        {
            Filter = "動画ファイル|*.mp4;*.mov;*.mkv;*.webm;*.avi|すべてのファイル|*.*",
            Title = "音声付きスライド動画を選択"
        };
        if (dialog.ShowDialog(this) == DialogResult.OK)
        {
            _videoTextBox.Text = dialog.FileName;
        }
    }

    private void BrowseModelFile()
    {
        using var dialog = new OpenFileDialog
        {
            Filter = "Whisperモデル|*.bin|すべてのファイル|*.*",
            Title = "Whisperモデルファイルを選択"
        };
        if (dialog.ShowDialog(this) == DialogResult.OK)
        {
            _modelPathTextBox.Text = dialog.FileName;
            UpdateModelStatus();
        }
    }

    private void BrowseOutputFolder(TextBox target)
    {
        using var dialog = new FolderBrowserDialog { Description = "フォルダを選択" };
        if (dialog.ShowDialog(this) == DialogResult.OK)
        {
            target.Text = dialog.SelectedPath;
        }
    }

    private void MainForm_DragEnter(object? sender, DragEventArgs e)
    {
        e.Effect = e.Data?.GetDataPresent(DataFormats.FileDrop) == true ? DragDropEffects.Copy : DragDropEffects.None;
    }

    private void MainForm_DragDrop(object? sender, DragEventArgs e)
    {
        if (e.Data?.GetData(DataFormats.FileDrop) is not string[] files || files.Length == 0)
        {
            return;
        }

        var projectFile = files.FirstOrDefault(file =>
            file.EndsWith(".json", StringComparison.OrdinalIgnoreCase) && File.Exists(file));
        if (projectFile is not null)
        {
            OpenProjectFile(projectFile);
            return;
        }

        var video = files.FirstOrDefault(file => new[]
        {
            ".mp4", ".mov", ".mkv", ".webm", ".avi"
        }.Contains(Path.GetExtension(file).ToLowerInvariant()));
        if (video is not null)
        {
            _videoTextBox.Text = video;
            ShowPage(0);
        }
    }

    private void SaveSettingsFromUi()
    {
        _settings.OutputDirectory = _outputTextBox.Text.Trim();
        _settings.ModelDirectory = _settingsModelDirectoryTextBox.Text.Trim();
        _settings.UseGpu = _useGpuCheckBox.Checked;
        _settings.SceneThreshold = (double)_sceneThresholdNumeric.Value;
        _settings.MinimumSlideSeconds = (double)_minimumSlideNumeric.Value;
        _settings.InitialPrompt = _promptTextBox.Text;
        _settings.ModelIndex = _modelComboBox.SelectedIndex;
        _settings.PdfMinimumFontSize = (double)_pdfMinimumFontNumeric.Value;
        _settings.Save();
    }

    private void MainForm_FormClosing(object? sender, FormClosingEventArgs e)
    {
        if (_busy)
        {
            MessageBox.Show(this, "処理中です。完了またはキャンセルしてから終了してください。", "終了できません",
                MessageBoxButtons.OK, MessageBoxIcon.Warning);
            e.Cancel = true;
            return;
        }

        if (_dirty && _project is not null)
        {
            var result = MessageBox.Show(this, "変更が保存されていません。保存しますか？", "未保存の変更",
                MessageBoxButtons.YesNoCancel, MessageBoxIcon.Question);
            if (result == DialogResult.Cancel)
            {
                e.Cancel = true;
                return;
            }
            if (result == DialogResult.Yes)
            {
                SaveProject();
                if (_dirty)
                {
                    e.Cancel = true;
                    return;
                }
            }
        }

        SaveSettingsFromUi();
    }

    private void UpdateTitleBarTheme()
    {
        if (!IsHandleCreated)
        {
            return;
        }
        var dark = _settings.DarkMode ? 1 : 0;
        DwmSetWindowAttribute(Handle, 20, ref dark, sizeof(int));
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    private static string MakeSafeFolderName(string name)
    {
        foreach (var character in Path.GetInvalidFileNameChars())
        {
            name = name.Replace(character, '_');
        }
        return string.IsNullOrWhiteSpace(name) ? "output" : name;
    }

    private static string FormatSeconds(double seconds)
    {
        var time = TimeSpan.FromSeconds(Math.Max(0, seconds));
        return time.Hours > 0
            ? $"{(int)time.TotalHours}:{time.Minutes:D2}:{time.Seconds:D2}"
            : $"{time.Minutes}:{time.Seconds:D2}";
    }
}
