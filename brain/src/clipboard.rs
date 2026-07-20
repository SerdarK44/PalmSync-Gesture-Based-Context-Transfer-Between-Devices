use clipboard_win::{formats, get_clipboard, set_clipboard};

pub fn get_text() -> anyhow::Result<String> {
    let text: String =
        get_clipboard(formats::Unicode).map_err(|e| anyhow::anyhow!("clipboard read: {}", e))?;
    Ok(text)
}

pub fn set_text(content: &str) -> anyhow::Result<()> {
    set_clipboard(formats::Unicode, content)
        .map_err(|e| anyhow::anyhow!("clipboard write: {}", e))?;
    Ok(())
}
