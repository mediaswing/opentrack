//! A report as a Word document (.docx). Headings use Word's Heading 1 and
//! Heading 2 styles, so they show in the Navigation pane and screen readers
//! can jump between them. Each table's first row is marked as its header,
//! repeated on every page, and the table has a description for screen readers.

use super::zip::zip;
use super::{Block, Report, Table, xml_escape};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// A4, with 2 cm margins, in twentieths of a point.
const PAGE_WIDTH: u32 = 11906;
const PAGE_HEIGHT: u32 = 16838;
const MARGIN: u32 = 1134;

/// The report as the bytes of a .docx file.
pub fn docx(report: &Report, created: chrono::NaiveDate) -> Vec<u8> {
    let document = document_xml(report);
    let styles = styles_xml(&report.language);
    let core = core_xml(report, created);
    let parts: [(&str, &[u8]); 7] = [
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/styles.xml", styles.as_bytes()),
        ("docProps/core.xml", core.as_bytes()),
        ("docProps/app.xml", APP.as_bytes()),
    ];
    zip(&parts)
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

const APP: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Open Budget Tracking</Application></Properties>"#;

fn core_xml(report: &Report, created: chrono::NaiveDate) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>{}</dc:creator><dc:language>{}</dc:language><dcterms:created xsi:type="dcterms:W3CDTF">{}T00:00:00Z</dcterms:created></cp:coreProperties>"#,
        xml_escape(&report.title),
        xml_escape(crate::APP_TITLE),
        xml_escape(&report.language),
        created.format("%Y-%m-%d"),
    )
}

fn styles_xml(language: &str) -> String {
    let heading = |id: &str, name: &str, level: u32, half_points: u32| {
        format!(
            r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="{before}" w:after="120"/><w:outlineLvl w:val="{level}"/></w:pPr><w:rPr><w:b/><w:sz w:val="{half_points}"/><w:szCs w:val="{half_points}"/></w:rPr></w:style>"#,
            before = if level == 0 { 0 } else { 360 },
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="{language}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>{}{}<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:pPr><w:spacing w:before="40" w:after="40" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:left w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:right w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="808080"/></w:tblBorders></w:tblPr></w:style></w:styles>"#,
        heading("Heading1", "heading 1", 0, 36),
        heading("Heading2", "heading 2", 1, 28),
    )
}

fn run(text: &str, bold: bool) -> String {
    let props = if bold { "<w:rPr><w:b/><w:bCs/></w:rPr>" } else { "" };
    format!(r#"<w:r>{props}<w:t xml:space="preserve">{}</w:t></w:r>"#, xml_escape(text))
}

fn paragraph(style: Option<&str>, text: &str) -> String {
    let props = style.map_or_else(String::new, |s| format!(r#"<w:pPr><w:pStyle w:val="{s}"/></w:pPr>"#));
    format!("<w:p>{props}{}</w:p>", run(text, false))
}

fn table(t: &Table) -> String {
    let content = PAGE_WIDTH - 2 * MARGIN;
    let widths: Vec<u32> = t.columns.iter().map(|c| (c.width * content as f32).round() as u32).collect();
    let mut out = format!(
        r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="{content}" w:type="dxa"/><w:tblLayout w:type="fixed"/><w:tblLook w:val="0420" w:firstRow="1" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="1" w:noVBand="1"/><w:tblCaption w:val="{}"/></w:tblPr><w:tblGrid>"#,
        xml_escape(&t.summary)
    );
    for w in &widths {
        out.push_str(&format!(r#"<w:gridCol w:w="{w}"/>"#));
    }
    out.push_str("</w:tblGrid>");
    let header: Vec<String> = t.columns.iter().map(|c| c.title.to_owned()).collect();
    for (i, row) in std::iter::once(&header).chain(&t.rows).enumerate() {
        let is_header = i == 0;
        // The header row repeats on each page, and no row breaks across two.
        out.push_str(if is_header { "<w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>" } else { "<w:tr><w:trPr><w:cantSplit/></w:trPr>" });
        for ((cell, column), width) in row.iter().zip(&t.columns).zip(&widths) {
            let align = if column.numeric { r#"<w:jc w:val="right"/>"# } else { "" };
            out.push_str(&format!(
                r#"<w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/></w:tcPr><w:p><w:pPr>{align}</w:pPr>{}</w:p></w:tc>"#,
                run(cell, is_header)
            ));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}

fn document_xml(report: &Report) -> String {
    let mut body = String::new();
    for block in &report.blocks {
        body.push_str(&match block {
            Block::Title(text) => paragraph(Some("Heading1"), text),
            Block::Heading(text) => paragraph(Some("Heading2"), text),
            Block::Paragraph(text) => paragraph(None, text),
            Block::Table(t) => table(t),
        });
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="{PAGE_WIDTH}" w:h="{PAGE_HEIGHT}"/><w:pgMar w:top="{MARGIN}" w:right="{MARGIN}" w:bottom="{MARGIN}" w:left="{MARGIN}" w:header="567" w:footer="567" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::{d, gb, sample};
    use super::super::{Period, build, zip::unzip};
    use super::*;

    #[test]
    fn a_word_report_has_its_parts_headings_and_tables() {
        let report = build(&sample(), Period::All, &gb(), d("2026-11-20"));
        let parts = unzip(&docx(&report, d("2026-11-20")));
        let names: Vec<&str> = parts.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names[0], "[Content_Types].xml", "Word wants this first");
        let part = |name: &str| String::from_utf8(parts.iter().find(|(n, _)| n == name).unwrap().1.clone()).unwrap();
        let document = part("word/document.xml");
        assert!(document.contains(r#"<w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t xml:space="preserve">Spending Report: All Spending</w:t>"#));
        assert_eq!(document.matches(r#"<w:pStyle w:val="Heading2"/>"#).count(), 3);
        assert_eq!(document.matches("<w:tblHeader/>").count(), 3, "every table has a header row");
        assert!(document.contains("Cinema &amp; &lt;popcorn&gt;"));
        assert!(document.contains("Café crème £ € Łódź"));
        assert!(document.contains(r#"<w:tblCaption w:val="Spending by category, largest first"#));
        assert!(part("docProps/core.xml").contains("<dc:title>Spending Report: All Spending</dc:title>"));
        assert!(part("word/styles.xml").contains(r#"<w:lang w:val="en-GB"/>"#));
    }
}
