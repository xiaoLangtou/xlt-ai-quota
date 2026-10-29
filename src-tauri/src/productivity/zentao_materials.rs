//! 将禅道附件内容整理为摘要输入；解析失败时明确报错。
use base64::Engine as _;
use std::io::{Cursor, Read};

#[derive(Default)]
pub struct Materials {
    pub text: String,
    pub images: Vec<String>,
    expanded: usize,
}

impl Materials {
    pub fn add(&mut self, name: &str, bytes: &[u8], depth: usize) -> Result<(), String> {
        self.expanded += bytes.len();
        if self.expanded > 20 * 1024 * 1024 { return Err("附件展开内容超过 20MB".into()); }
        let extension = name.rsplit('.').next().unwrap_or("").to_lowercase();
        if let Some(mime) = match extension.as_str() {
            "png" => Some("image/png"), "jpg" | "jpeg" => Some("image/jpeg"),
            "gif" => Some("image/gif"), "webp" => Some("image/webp"), _ => None,
        } {
            if bytes.len() > 8 * 1024 * 1024 { return Err(format!("图片 {name} 超过 8MB")); }
            self.images.push(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
            self.text.push_str(&format!("\n{name}：图片，见随附图像 {}\n", self.images.len()));
            return Ok(());
        }
        let content = match extension.as_str() {
            "pdf" => {
                let text = pdf_extract::extract_text_from_mem(bytes).map_err(|_| format!("无法解析 PDF：{name}"))?;
                if text.trim().is_empty() { return Err(format!("PDF {name} 没有可提取正文，请将扫描页以图片附件提供")); }
                text
            },
            "docx" | "zip" => {
                if depth > 1 || (extension == "zip" && depth > 0) { return Err(format!("不支持嵌套压缩包：{name}")); }
                let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| format!("无法解析附件：{name}"))?;
                if archive.len() > 100 { return Err(format!("附件 {name} 内文件超过 100 个")); }
                for index in 0..archive.len() {
                    let mut file = archive.by_index(index).map_err(|_| format!("无法读取附件条目：{name}"))?;
                    let path = file.name().to_owned();
                    if file.is_dir() || path.split('/').any(|part| part == "__MACOSX" || part == ".DS_Store" || part.starts_with("._")) { continue; }
                    if extension == "docx" && path != "word/document.xml" && !path.starts_with("word/media/") { continue; }
                    let mut data = Vec::new();
                    file.by_ref().take(10 * 1024 * 1024 + 1).read_to_end(&mut data).map_err(|_| format!("附件条目读取失败：{path}"))?;
                    if data.len() > 10 * 1024 * 1024 { return Err(format!("附件条目超过 10MB：{path}")); }
                    if extension == "docx" && path == "word/document.xml" {
                        self.expanded += data.len();
                        if self.expanded > 20 * 1024 * 1024 { return Err("附件展开内容超过 20MB".into()); }
                        let xml = std::str::from_utf8(&data).map_err(|_| "DOCX 正文编码无效")?;
                        self.text.push_str(&super::document::docx_text(xml)?);
                    } else { self.add(&format!("{name}/{path}"), &data, depth + 1)?; }
                }
                String::new()
            }
            "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "xml" | "yml" | "yaml" | "html" | "htm" | "log" | "css" | "js" | "ts" | "py" | "java" | "go" | "rs" | "sql" | "sh" | "ini" | "conf" => {
                std::str::from_utf8(bytes).map_err(|_| format!("附件不是 UTF-8 文本：{name}"))?.to_owned()
            }
            _ => return Err(format!("暂不支持读取附件 {name}，请转为 PDF、DOCX、文本、ZIP 或 PNG/JPEG/GIF/WebP 图片")),
        };
        self.text.push_str(&format!("\n{name}\n{content}\n"));
        if self.text.chars().count() > 100_000 { return Err("附件正文超过十万字，请缩小附件范围".into()); }
        if self.images.len() > 24 { return Err("附件图片超过二十四张".into()); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn reads_docx_text_and_embedded_images_inside_zip() {
        let docx = archive(&[("word/document.xml", b"<w:document><w:p><w:r><w:t>changes</w:t></w:r></w:p></w:document>"), ("word/media/image.png", &[0x89, 0x50])]);
        let zip = archive(&[("requirements.docx", &docx), ("api.txt", b"backend change")]);
        let mut materials = Materials::default();
        materials.add("files.zip", &zip, 0).unwrap();
        assert!(materials.text.contains("changes"));
        assert!(materials.text.contains("backend change"));
        assert_eq!(materials.images.len(), 1);
        assert!(Materials::default().add("nested.zip", &archive(&[("inner.zip", &zip)]), 0).is_err());
    }
    #[test]
    fn keeps_text_and_image_bytes_and_rejects_unknown_binary() {
        let mut materials = Materials::default();
        materials.add("需求.md", "增加按钮".as_bytes(), 0).unwrap();
        materials.add("截图.png", &[0x89, 0x50], 0).unwrap();
        assert!(materials.text.contains("增加按钮"));
        assert_eq!(materials.images[0], "data:image/png;base64,iVA=");
        assert!(materials.add("未知.exe", &[0, 1], 0).is_err());
    }
}
