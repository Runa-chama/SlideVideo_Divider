namespace SlideVideoDivider.Core;

public sealed class EditAction
{
    public required string Description { get; init; }
    public required Action Undo { get; init; }
    public required Action Redo { get; init; }
}

public sealed class EditHistory
{
    private readonly Stack<EditAction> _undoStack = new();
    private readonly Stack<EditAction> _redoStack = new();
    private readonly int _capacity;

    public EditHistory(int capacity = 100)
    {
        _capacity = Math.Max(1, capacity);
    }

    public event EventHandler? Changed;

    public bool CanUndo => _undoStack.Count > 0;
    public bool CanRedo => _redoStack.Count > 0;
    public string? UndoDescription => _undoStack.TryPeek(out var action) ? action.Description : null;
    public string? RedoDescription => _redoStack.TryPeek(out var action) ? action.Description : null;

    public void Push(EditAction action)
    {
        _undoStack.Push(action);
        while (_undoStack.Count > _capacity)
        {
            _undoStack.Pop();
        }
        _redoStack.Clear();
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void Undo()
    {
        if (!_undoStack.TryPop(out var action))
        {
            return;
        }

        action.Undo();
        _redoStack.Push(action);
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void Redo()
    {
        if (!_redoStack.TryPop(out var action))
        {
            return;
        }

        action.Redo();
        _undoStack.Push(action);
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void Clear()
    {
        _undoStack.Clear();
        _redoStack.Clear();
        Changed?.Invoke(this, EventArgs.Empty);
    }
}
