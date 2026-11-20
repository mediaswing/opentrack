//! A report as a tagged PDF, which screen readers can read properly: it has a
//! structure tree, so headings are headings and tables have header cells; a
//! title and a language; and its text maps back to Unicode, so it can be read,
//! searched and copied. Page numbers, rules and header rows repeated on later
//! pages are marked as decoration (artifacts), so they aren't read out as
//! content.
//!
//! The text is set in the Ubuntu fonts the window uses, embedded whole, so it
//! looks the same everywhere. Pages are A4.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

use chrono::NaiveDate;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use ttf_parser::{Face, GlyphId};

use super::{Block, Report, Table};

const PAGE_WIDTH: f32 = 595.28;
const PAGE_HEIGHT: f32 = 841.89;
/// 2 cm.
const MARGIN: f32 = 56.7;
const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const BODY: f32 = 10.5;
const TITLE: f32 = 20.0;
const HEADING: f32 = 14.0;
/// Line height, as a multiple of the font size.
const LEADING: f32 = 1.3;
/// Space around the text in a table cell.
const PAD: f32 = 4.0;

const REGULAR: usize = 0;
const BOLD: usize = 1;

struct Font {
    name: &'static str,
    data: &'static [u8],
    face: Face<'static>,
    /// Font units to thousandths of the font size.
    scale: f32,
    /// Every glyph drawn, with the character it stands for, if it has one.
    used: BTreeMap<u16, Option<char>>,
}

impl Font {
    fn new(name: &'static str, data: &'static [u8]) -> Self {
        let face = Face::parse(data, 0).expect("the bundled fonts can be read");
        let scale = 1000.0 / f32::from(face.units_per_em());
        Font { name, data, face, scale, used: BTreeMap::new() }
    }

    /// The glyph for `c`, or for a character that looks the same.
    fn glyph(&self, c: char) -> Option<u16> {
        let c = match c {
            '\u{a0}' | '\u{202f}' | '\u{2009}' => ' ',
            '\u{ffe5}' => '¥',
            c => c,
        };
        self.face.glyph_index(c).map(|g| g.0)
    }

    /// In thousandths of the font size.
    fn advance(&self, glyph: u16) -> f32 {
        f32::from(self.face.glyph_hor_advance(GlyphId(glyph)).unwrap_or(0)) * self.scale
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(self.glyph(c).unwrap_or(0))).sum::<f32>() * size / 1000.0
    }

    /// Whether every character has a glyph. If not, the text is given to
    /// screen readers separately, since a missing glyph can't be read.
    fn covers(&self, text: &str) -> bool {
        text.chars().all(|c| self.glyph(c).is_some())
    }

    /// `text` as glyph ids in hex, for a content stream, remembering each one.
    fn encode(&mut self, text: &str) -> String {
        let mut hex = String::with_capacity(text.len() * 4);
        for c in text.chars() {
            let glyph = self.glyph(c);
            let id = glyph.unwrap_or(0);
            let entry = self.used.entry(id).or_insert(None);
            if glyph.is_some() && entry.is_none() {
                *entry = Some(c);
            }
            let _ = write!(hex, "{id:04X}");
        }
        hex
    }

    /// Splits `text` into lines no wider than `width`, between words where
    /// possible, and within a word too long for a line.
    fn wrap(&self, text: &str, size: f32, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split(' ').filter(|w| !w.is_empty()) {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if self.width(&candidate, size) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for c in word.chars() {
                line.push(c);
                if self.width(&line, size) > width && line.chars().count() > 1 {
                    line.pop();
                    lines.push(std::mem::take(&mut line));
                    line.push(c);
                }
            }
        }
        if !line.is_empty() || lines.is_empty() {
            lines.push(line);
        }
        lines
    }
}

/// A child in the structure tree: another element, or some content on a page.
enum Kid {
    Element(usize),
    Content { page: usize, mcid: usize },
}

/// An element of the structure tree, such as a heading or a table cell.
struct Element {
    tag: &'static str,
    parent: Option<usize>,
    kids: Vec<Kid>,
}

#[derive(Default)]
struct Page {
    ops: String,
    /// The element each piece of marked content belongs to, by its number.
    owners: Vec<usize>,
}

struct Layout {
    fonts: [Font; 2],
    pages: Vec<Page>,
    elements: Vec<Element>,
    /// How far down the page the next thing goes.
    y: f32,
}

/// The structure tree's root element, which everything else is inside.
const DOCUMENT: usize = 0;

impl Layout {
    fn new() -> Self {
        let mut layout = Layout {
            fonts: [
                Font::new("Ubuntu-Medium", include_bytes!("../../assets/fonts/Ubuntu-Medium.ttf")),
                Font::new("Ubuntu-Bold", include_bytes!("../../assets/fonts/Ubuntu-Bold.ttf")),
            ],
            pages: Vec::new(),
            elements: vec![Element { tag: "Document", parent: None, kids: Vec::new() }],
            y: 0.0,
        };
        layout.new_page();
        layout
    }

    fn new_page(&mut self) {
        self.pages.push(Page::default());
        self.y = PAGE_HEIGHT - MARGIN;
    }

    fn at_top(&self) -> bool {
        self.y >= PAGE_HEIGHT - MARGIN
    }

    /// Starts a new page unless there is `height` left on this one.
    fn need(&mut self, height: f32) {
        if self.y - height < MARGIN && !self.at_top() {
            self.new_page();
        }
    }

    fn element(&mut self, tag: &'static str, parent: usize) -> usize {
        self.elements.push(Element { tag, parent: Some(parent), kids: Vec::new() });
        let id = self.elements.len() - 1;
        self.elements[parent].kids.push(Kid::Element(id));
        id
    }

    fn ops(&mut self) -> &mut String {
        &mut self.pages.last_mut().expect("there is always a page").ops
    }

    /// Draws content for `element`, marked so that screen readers find it in
    /// the structure tree. `text` is what it says.
    fn content(&mut self, element: usize, font: usize, text: &str, draw: impl FnOnce(&mut Self)) {
        let page = self.pages.len() - 1;
        let mcid = self.pages[page].owners.len();
        self.pages[page].owners.push(element);
        self.elements[element].kids.push(Kid::Content { page, mcid });
        let actual = if self.fonts[font].covers(text) { String::new() } else { format!(" /ActualText {}", text_string(text)) };
        let tag = self.elements[element].tag;
        let _ = writeln!(self.ops(), "/{tag} <</MCID {mcid}{actual}>> BDC");
        draw(self);
        self.ops().push_str("EMC\n");
    }

    /// Draws decoration that screen readers should skip.
    fn artifact(&mut self, kind: &str, draw: impl FnOnce(&mut Self)) {
        let _ = writeln!(self.ops(), "/Artifact <</Type /{kind}>> BDC");
        draw(self);
        self.ops().push_str("EMC\n");
    }

    fn text(&mut self, font: usize, size: f32, x: f32, baseline: f32, text: &str) {
        let hex = self.fonts[font].encode(text);
        let _ = writeln!(self.ops(), "BT /F{font} {} Tf {} {} Td <{hex}> Tj ET", n(size), n(x), n(baseline));
    }

    fn rule(&mut self, y: f32, grey: f32, width: f32) {
        self.artifact("Layout", |l| {
            let _ = writeln!(l.ops(), "{} G {} w {} {} m {} {} l S", n(grey), n(width), n(MARGIN), n(y), n(PAGE_WIDTH - MARGIN), n(y));
        });
    }

    /// A heading or paragraph, wrapped to the page's width. `keep` is room
    /// that must be left below it, so a heading isn't left alone at the
    /// bottom of a page.
    fn paragraph(&mut self, tag: &'static str, text: &str, font: usize, size: f32, before: f32, keep: f32) {
        let element = self.element(tag, DOCUMENT);
        let lines = self.fonts[font].wrap(text, size, CONTENT_WIDTH);
        let line_height = size * LEADING;
        if !self.at_top() {
            self.y -= before;
        }
        self.need(line_height * lines.len() as f32 + keep);
        for line in lines {
            self.need(line_height);
            let baseline = self.y - size;
            self.y -= line_height;
            self.content(element, font, &line, |l| l.text(font, size, MARGIN, baseline, &line));
        }
        self.y -= size * 0.5;
    }

    fn cell_lines(&self, cells: &[String], font: usize, widths: &[f32]) -> Vec<Vec<String>> {
        cells.iter().zip(widths).map(|(cell, w)| self.fonts[font].wrap(cell, BODY, w - 2.0 * PAD)).collect()
    }

    fn row_height(lines: &[Vec<String>]) -> f32 {
        lines.iter().map(Vec::len).max().unwrap_or(1) as f32 * BODY * LEADING + 2.0 * PAD
    }

    /// One row of a table. Without a `table` element, it is a header row
    /// repeated at the top of a page, which isn't read again.
    fn row(&mut self, t: &Table, cells: &[String], widths: &[f32], table: Option<usize>, header: bool) {
        let font = if header { BOLD } else { REGULAR };
        let lines = self.cell_lines(cells, font, widths);
        let top = self.y;
        let row = table.map(|table| self.element("TR", table));
        let mut x = MARGIN;
        for (i, (cell_lines, width)) in lines.iter().zip(widths).enumerate() {
            let numeric = t.columns[i].numeric;
            let draw = |l: &mut Self| {
                for (k, line) in cell_lines.iter().enumerate() {
                    let baseline = top - PAD - BODY * 0.95 - k as f32 * BODY * LEADING;
                    let left = if numeric { x + width - PAD - l.fonts[font].width(line, BODY) } else { x + PAD };
                    l.text(font, BODY, left, baseline, line);
                }
            };
            match row {
                Some(row) => {
                    let cell = self.element(if header { "TH" } else { "TD" }, row);
                    self.content(cell, font, &cells[i], draw);
                }
                None => self.artifact("Pagination /Subtype /Header", draw),
            }
            x += width;
        }
        self.y = top - Self::row_height(&lines);
        if header {
            self.rule(self.y, 0.0, 0.8);
        } else {
            self.rule(self.y, 0.6, 0.3);
        }
    }

    fn table(&mut self, t: &Table) {
        let widths: Vec<f32> = t.columns.iter().map(|c| c.width * CONTENT_WIDTH).collect();
        let header: Vec<String> = t.columns.iter().map(|c| c.title.to_owned()).collect();
        let header_height = Self::row_height(&self.cell_lines(&header, BOLD, &widths));
        let first = t.rows.first().map_or(0.0, |r| Self::row_height(&self.cell_lines(r, REGULAR, &widths)));
        self.need(header_height + first);
        let table = self.element("Table", DOCUMENT);
        self.row(t, &header, &widths, Some(table), true);
        for cells in &t.rows {
            let height = Self::row_height(&self.cell_lines(cells, REGULAR, &widths));
            if self.y - height < MARGIN {
                self.new_page();
                self.row(t, &header, &widths, None, true);
            }
            self.row(t, cells, &widths, Some(table), false);
        }
        self.y -= BODY;
    }

    /// "Page 1 of 3" at the foot of each page.
    fn page_numbers(&mut self) {
        let count = self.pages.len();
        let pages = std::mem::take(&mut self.pages);
        for (i, page) in pages.into_iter().enumerate() {
            self.pages.push(page);
            let text = format!("Page {} of {count}", i + 1);
            let x = (PAGE_WIDTH - self.fonts[REGULAR].width(&text, 9.0)) / 2.0;
            self.artifact("Pagination /Subtype /Footer", |l| l.text(REGULAR, 9.0, x, MARGIN / 2.0, &text));
        }
    }
}

/// A number for a content stream.
fn n(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Text in a PDF string: UTF-16, in hex.
fn text_string(text: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(hex, "{unit:04X}");
    }
    hex.push('>');
    hex
}

/// Builds the file, numbering objects as it goes.
struct Writer {
    out: Vec<u8>,
    /// Each object's offset in the file, by its number less one.
    offsets: Vec<usize>,
}

impl Writer {
    fn reserve(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len()
    }

    fn object(&mut self, id: usize, body: &str) {
        self.offsets[id - 1] = self.out.len();
        let _ = write!(self.out, "{id} 0 obj\n{body}\nendobj\n");
    }

    fn stream(&mut self, id: usize, dictionary: &str, data: &[u8], compress: bool) {
        let packed;
        let (data, filter) = if compress {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).expect("writing to memory can't fail");
            packed = encoder.finish().expect("writing to memory can't fail");
            (&packed[..], " /Filter /FlateDecode")
        } else {
            (data, "")
        };
        self.offsets[id - 1] = self.out.len();
        let _ = write!(self.out, "{id} 0 obj\n<< {dictionary} /Length {}{filter} >>\nstream\n", data.len());
        self.out.extend_from_slice(data);
        self.out.extend_from_slice(b"\nendstream\nendobj\n");
    }
}

/// The objects that describe one embedded font.
fn write_font(w: &mut Writer, font: &Font, ids: [usize; 5]) {
    let [type0, cid, descriptor, file, to_unicode] = ids;
    let face = &font.face;
    let s = |v: i16| (f32::from(v) * font.scale).round() as i32;
    let bbox = face.global_bounding_box();
    let cap = face.capital_height().unwrap_or(face.ascender());
    let mut widths = String::new();
    for &glyph in font.used.keys() {
        let _ = write!(widths, "{glyph} [{}] ", font.advance(glyph).round() as i32);
    }
    w.object(
        type0,
        &format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {to_unicode} 0 R >>",
            font.name
        ),
    );
    w.object(
        cid,
        &format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor} 0 R /DW 1000 /W [{widths}] /CIDToGIDMap /Identity >>",
            font.name
        ),
    );
    w.object(
        descriptor,
        &format!(
            "<< /Type /FontDescriptor /FontName /{} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV {} /FontFile2 {file} 0 R >>",
            font.name,
            s(bbox.x_min),
            s(bbox.y_min),
            s(bbox.x_max),
            s(bbox.y_max),
            s(face.ascender()),
            s(face.descender()),
            s(cap),
            if font.name.ends_with("Bold") { 140 } else { 90 },
        ),
    );
    w.stream(file, &format!("/Length1 {}", font.data.len()), font.data, true);

    // Which characters each glyph stands for, so the text can be read out.
    let mapped: Vec<(u16, char)> = font.used.iter().filter_map(|(&g, c)| c.map(|c| (g, c))).collect();
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in mapped.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (glyph, c) in chunk {
            let unicode: String = c.encode_utf16(&mut [0; 2]).iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(cmap, "<{glyph:04X}> <{unicode}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    w.stream(to_unicode, "", cmap.as_bytes(), true);
}

/// The report as the bytes of a PDF file, made on `made`.
pub fn pdf(report: &Report, made: NaiveDate) -> Vec<u8> {
    let mut layout = Layout::new();
    for block in &report.blocks {
        match block {
            Block::Title(text) => layout.paragraph("H1", text, BOLD, TITLE, 0.0, BODY * 3.0),
            Block::Heading(text) => layout.paragraph("H2", text, BOLD, HEADING, HEADING, BODY * 6.0),
            Block::Paragraph(text) => layout.paragraph("P", text, REGULAR, BODY, BODY * 0.3, 0.0),
            Block::Table(t) => layout.table(t),
        }
    }
    layout.page_numbers();

    let mut w = Writer { out: b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec(), offsets: Vec::new() };
    let catalog = w.reserve();
    let pages_id = w.reserve();
    let info = w.reserve();
    let metadata = w.reserve();
    let tree = w.reserve();
    let font_ids: Vec<[usize; 5]> = (0..2).map(|_| [(); 5].map(|()| w.reserve())).collect();
    let page_ids: Vec<(usize, usize)> = layout.pages.iter().map(|_| (w.reserve(), w.reserve())).collect();
    let element_ids: Vec<usize> = layout.elements.iter().map(|_| w.reserve()).collect();

    w.object(
        catalog,
        &format!(
            "<< /Type /Catalog /Pages {pages_id} 0 R /StructTreeRoot {tree} 0 R /MarkInfo << /Marked true >> /Lang {} /ViewerPreferences << /DisplayDocTitle true >> /Metadata {metadata} 0 R >>",
            text_string(&report.language)
        ),
    );
    let kids: Vec<String> = page_ids.iter().map(|(page, _)| format!("{page} 0 R")).collect();
    w.object(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()));
    let version = env!("CARGO_PKG_VERSION");
    w.object(
        info,
        &format!(
            "<< /Title {} /Creator {} /Producer {} /CreationDate (D:{}000000) >>",
            text_string(&report.title),
            text_string(crate::APP_TITLE),
            text_string(&format!("{} {version}", crate::APP_TITLE)),
            made.format("%Y%m%d")
        ),
    );
    let title = super::xml_escape(&report.title);
    let xmp = format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title><dc:language><rdf:Bag><rdf:li>{}</rdf:li></rdf:Bag></dc:language></rdf:Description></rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>",
        super::xml_escape(&report.language)
    );
    w.stream(metadata, "/Type /Metadata /Subtype /XML", xmp.as_bytes(), false);

    // The structure tree, and the map from each page's marked content back to it.
    let mut parent_tree = String::new();
    for (i, page) in layout.pages.iter().enumerate() {
        let owners: Vec<String> = page.owners.iter().map(|&e| format!("{} 0 R", element_ids[e])).collect();
        let _ = write!(parent_tree, "{i} [{}] ", owners.join(" "));
    }
    w.object(
        tree,
        &format!(
            "<< /Type /StructTreeRoot /K [{} 0 R] /ParentTree << /Nums [{parent_tree}] >> /ParentTreeNextKey {} >>",
            element_ids[DOCUMENT],
            layout.pages.len()
        ),
    );
    for (element, &id) in layout.elements.iter().zip(&element_ids) {
        let parent = element.parent.map_or(tree, |p| element_ids[p]);
        let kids: Vec<String> = element
            .kids
            .iter()
            .map(|kid| match kid {
                Kid::Element(e) => format!("{} 0 R", element_ids[*e]),
                Kid::Content { page, mcid } => format!("<< /Type /MCR /Pg {} 0 R /MCID {mcid} >>", page_ids[*page].0),
            })
            .collect();
        let attributes = if element.tag == "TH" { " /A << /O /Table /Scope /Column >>" } else { "" };
        w.object(id, &format!("<< /Type /StructElem /S /{} /P {parent} 0 R /K [{}]{attributes} >>", element.tag, kids.join(" ")));
    }

    for (i, (page, (page_id, content_id))) in layout.pages.iter().zip(&page_ids).enumerate() {
        w.object(
            *page_id,
            &format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F0 {} 0 R /F1 {} 0 R >> >> /Contents {content_id} 0 R /StructParents {i} /Tabs /S >>",
                n(PAGE_WIDTH),
                n(PAGE_HEIGHT),
                font_ids[REGULAR][0],
                font_ids[BOLD][0]
            ),
        );
        w.stream(*content_id, "", page.ops.as_bytes(), true);
    }
    for (font, ids) in layout.fonts.iter().zip(&font_ids) {
        write_font(&mut w, font, *ids);
    }

    let xref = w.out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", w.offsets.len() + 1);
    for offset in &w.offsets {
        let _ = writeln!(tail, "{offset:010} 00000 n ");
    }
    let id = format!("{:08X}{:08X}", crc32fast::hash(&w.out), w.out.len());
    let _ = write!(
        tail,
        "trailer\n<< /Size {} /Root {catalog} 0 R /Info {info} 0 R /ID [<{id}> <{id}>] >>\nstartxref\n{xref}\n%%EOF\n",
        w.offsets.len() + 1
    );
    w.out.extend_from_slice(tail.as_bytes());
    w.out
}

#[cfg(test)]
mod tests {
    use super::super::tests::{d, gb, sample};
    use super::super::{Period, build};
    use super::*;

    fn text(pdf: &[u8]) -> String {
        String::from_utf8_lossy(pdf).into_owned()
    }

    /// Every object the cross-reference table lists is where it says. Works on
    /// bytes, since offsets count bytes and the streams are binary.
    fn check_xref(pdf: &[u8]) -> usize {
        let tail = text(&pdf[pdf.len() - 64..]);
        let start: usize = tail.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[start..].starts_with(b"xref\n0 "), "startxref points at the table");
        let table = text(&pdf[start..]);
        let mut lines = table.lines().skip(1);
        let count: usize = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
        lines.next();
        for id in 1..count {
            let entry = lines.next().unwrap();
            assert_eq!(entry.len() + 1, 20, "xref entries are 20 bytes");
            let offset: usize = entry[..10].parse().unwrap();
            assert!(pdf[offset..].starts_with(format!("{id} 0 obj\n").as_bytes()), "object {id} is at {offset}");
        }
        count - 1
    }

    #[test]
    fn a_pdf_report_is_tagged_and_well_formed() {
        let report = build(&sample(), Period::All, &gb(), d("2026-11-20"));
        let pdf = pdf(&report, d("2026-11-20"));
        assert!(pdf.starts_with(b"%PDF-1.7\n"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        check_xref(&pdf);
        let s = text(&pdf);
        assert!(s.contains("/MarkInfo << /Marked true >>"));
        assert!(s.contains(&format!("/Lang {}", text_string("en-GB"))));
        assert!(s.contains("/DisplayDocTitle true"));
        assert!(s.contains(&format!("/Title {}", text_string("Spending Report: All Spending"))));
        assert!(s.contains("<rdf:li xml:lang=\"x-default\">Spending Report: All Spending</rdf:li>"));
        assert_eq!(s.matches("/S /H1 ").count(), 1);
        assert_eq!(s.matches("/S /H2 ").count(), 3);
        assert_eq!(s.matches("/S /Table ").count(), 3);
        // Four header cells, three, and four.
        assert_eq!(s.matches("/S /TH /P").count(), 11);
        assert_eq!(s.matches("/S /TD /P").count(), 2 * 4 + 2 * 3 + 4 * 4);
        assert_eq!(s.matches("/Type /Page ").count(), 1);
    }

    #[test]
    fn long_reports_go_on_to_more_pages_with_the_header_repeated() {
        let mut entries = sample();
        for i in 0..200 {
            let mut e = entries[1].clone();
            e.line = 10 + i;
            e.name = format!("Item {i} with a name long enough to wrap onto a second line in its cell, which it does");
            entries.push(e);
        }
        let report = build(&entries, Period::All, &gb(), d("2026-11-20"));
        let pdf = pdf(&report, d("2026-11-20"));
        check_xref(&pdf);
        let s = text(&pdf);
        let pages = s.matches("/Type /Page ").count();
        assert!(pages > 5, "{pages} pages");
        assert_eq!(s.matches("/S /TR ").count(), (1 + 2) + (1 + 2) + (1 + 204), "repeated header rows aren't in the structure tree");
        assert!(s.contains(&format!("/ParentTreeNextKey {pages}")));
    }

    #[test]
    fn every_character_can_be_read_back() {
        let mut layout = Layout::new();
        layout.paragraph("P", "Café £12 € Łódź ₹ \u{ffe5}", REGULAR, BODY, 0.0, 0.0);
        let font = &layout.fonts[REGULAR];
        for c in "Café£€Łódź".chars() {
            let glyph = font.glyph(c).unwrap();
            assert_eq!(font.used[&glyph], Some(c), "{c}");
        }
        assert!(font.glyph('\u{ffe5}').is_some(), "the full-width yen is drawn as a yen");
    }

    #[test]
    fn text_wraps_between_words_and_within_long_ones() {
        let layout = Layout::new();
        let font = &layout.fonts[REGULAR];
        let lines = font.wrap("one two three four five six seven", BODY, 60.0);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.join(" "), "one two three four five six seven");
        let long = "x".repeat(200);
        let lines = font.wrap(&long, BODY, 60.0);
        assert!(lines.iter().all(|l| font.width(l, BODY) <= 60.0), "{lines:?}");
        assert_eq!(lines.concat(), long);
        assert_eq!(font.wrap("", BODY, 60.0), [""]);
    }
}
