use crate::productivity::types::MAX_DOCUMENT_BYTES;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::io::Read;
use std::path::Path;

const SUPPORTED: &str = "仅支持 Markdown、TXT、DOCX 和 PDF 文件";

/// 在本机读取并提取文档正文，返回 (文件名, 正文, 字符数)。
/// 只读取用户明确选择的路径，不保存原始文件，也不向任何服务发送本机路径。
pub fn read_document(path: &str) -> Result<(String, String, usize), String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() {
        return Err("请选择要拆解的文档".to_owned());
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "无法读取文件名".to_owned())?
        .to_owned();
    let metadata = fs::metadata(path).map_err(|error| format!("无法读取文档：{error}"))?;
    if !metadata.is_file() {
        return Err("请选择有效的文档文件".to_owned());
    }
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(format!(
            "文件超过 {} MB 限制（当前 {:.1} MB）",
            MAX_DOCUMENT_BYTES / 1024 / 1024,
            metadata.len() as f64 / 1024.0 / 1024.0
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let content = match extension.as_str() {
        "md" | "markdown" | "txt" => read_plain_text(path)?,
        "docx" => read_docx(path)?,
        "pdf" => read_pdf(path)?,
        "" => return Err(SUPPORTED.to_owned()),
        _ => return Err(SUPPORTED.to_owned()),
    };
    let content = content.trim();
    if content.is_empty() {
        return Err("未能从文档中提取到正文内容".to_owned());
    }
    let content = content.to_owned();
    let char_count = content.chars().count();
    Ok((name, content, char_count))
}

fn read_plain_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取文档：{error}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_docx(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|error| format!("无法读取文档：{error}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("无法解析 DOCX：{error}"))?;
    let mut xml = String::new();
    {
        let mut document = archive
            .by_name("word/document.xml")
            .map_err(|_| "DOCX 缺少 word/document.xml".to_owned())?;
        document
            .read_to_string(&mut xml)
            .map_err(|error| format!("无法读取 DOCX 正文：{error}"))?;
    }
    docx_text(&xml)
}

/// 从 word/document.xml 中抽取纯文本：`<w:t>` 为文本节点，`<w:tab>` 制表，`<w:br>`/`<w:p>` 换行。
pub(super) fn docx_text(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut output = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match event.local_name().as_ref() {
                b"t" => in_text = true,
                b"tab" => output.push('\t'),
                b"br" => output.push('\n'),
                _ => {}
            },
            Ok(Event::End(event)) => match event.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => output.push('\n'),
                _ => {}
            },
            Ok(Event::Empty(event)) => match event.local_name().as_ref() {
                b"tab" => output.push('\t'),
                b"br" => output.push('\n'),
                _ => {}
            },
            Ok(Event::Text(event)) => {
                if in_text {
                    let raw = String::from_utf8_lossy(event.as_ref());
                    let text = quick_xml::escape::unescape(&raw)
                        .map_err(|error| format!("无法解析 DOCX 文本：{error}"))?;
                    output.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("无法解析 DOCX：{error}")),
            _ => {}
        }
    }
    Ok(output)
}

fn read_pdf(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取文档：{error}"))?;
    pdf_extract::extract_text_from_mem(&bytes).map_err(|error| format!("无法解析 PDF：{error}"))
}

#[cfg(test)]
mod tests {
    use super::docx_text;

    #[test]
    fn extracts_docx_paragraph_text_and_breaks() {
        let xml = r#"<?xml version="1.0"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p><w:r><w:t>第一段</w:t></w:r></w:p>
            <w:p><w:r><w:t>第二</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>段</w:t></w:r></w:p>
          </w:body>
        </w:document>"#;
        let text = docx_text(xml).expect("解析 docx");
        assert!(text.contains("第一段"));
        assert!(text.contains("第二\t段"));
        assert_eq!(text.matches('\n').count(), 2);
    }
}
