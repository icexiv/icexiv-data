//! Excel header metadata and page-path generation; no I/O or schema-name lookup.

use super::{Error, Result, read, table_end};
use binrw::{BinRead, Endian};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    Default,
    Subrows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    None,
    Japanese,
    English,
    German,
    French,
    ChineseSimplified,
    ChineseTraditional,
    Korean,
}

impl Language {
    fn from_id(id: u16) -> Result<Self> {
        Ok(match id {
            0 => Self::None,
            1 => Self::Japanese,
            2 => Self::English,
            3 => Self::German,
            4 => Self::French,
            5 => Self::ChineseSimplified,
            6 => Self::ChineseTraditional,
            7 => Self::Korean,
            _ => return Err(Error::UnsupportedLanguage(id)),
        })
    }

    /// Filename suffix, including the underscore (empty for unlocalized data).
    pub fn suffix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Japanese => "_ja",
            Self::English => "_en",
            Self::German => "_de",
            Self::French => "_fr",
            Self::ChineseSimplified => "_chs",
            Self::ChineseTraditional => "_cht",
            Self::Korean => "_ko",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnType {
    String,
    Bool,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
    Int64,
    UInt64,
    PackedBool(u8),
}

impl ColumnType {
    fn from_id(id: u16) -> Result<Self> {
        Ok(match id {
            0 => Self::String,
            1 => Self::Bool,
            2 => Self::Int8,
            3 => Self::UInt8,
            4 => Self::Int16,
            5 => Self::UInt16,
            6 => Self::Int32,
            7 => Self::UInt32,
            9 => Self::Float32,
            10 => Self::Int64,
            11 => Self::UInt64,
            0x19..=0x20 => Self::PackedBool((id - 0x19) as u8),
            _ => return Err(Error::UnsupportedColumnType(id)),
        })
    }

    /// Width in the fixed-size row area (strings store a four-byte offset).
    pub fn width(self) -> usize {
        match self {
            Self::Bool | Self::Int8 | Self::UInt8 | Self::PackedBool(_) => 1,
            Self::Int16 | Self::UInt16 => 2,
            Self::String | Self::Int32 | Self::UInt32 | Self::Float32 => 4,
            Self::Int64 | Self::UInt64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Column {
    pub data_type: ColumnType,
    pub offset: u16,
}

/// A half-open range of possible parent row IDs, not a count of populated rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Page {
    pub start_id: u32,
    pub row_count: u32,
}

impl Page {
    pub fn contains(self, id: u32) -> bool {
        u64::from(id) >= u64::from(self.start_id)
            && u64::from(id) < u64::from(self.start_id) + u64::from(self.row_count)
    }
}

#[derive(BinRead)]
#[br(big, magic = b"EXHF")]
struct HeaderRaw {
    version: u16,
    fixed_size: u16,
    columns: u16,
    pages: u16,
    languages: u16,
    _unknown1: u16,
    _unknown2: u8,
    variant: u8,
    _unknown3: u16,
    row_count: u32,
    _reserved: [u32; 2],
}

#[derive(BinRead)]
#[br(big)]
struct ColumnRaw {
    kind: u16,
    offset: u16,
}

#[derive(BinRead)]
#[br(big)]
struct PageRaw {
    start_id: u32,
    row_count: u32,
}

/// Owned metadata parsed from EXH v3. Column names are not part of this format.
#[derive(Clone, Debug)]
pub struct Exh {
    fixed_size: u16,
    variant: Variant,
    row_count: u32,
    columns: Vec<Column>,
    pages: Vec<Page>,
    languages: Vec<Language>,
}

impl Exh {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let raw: HeaderRaw = read(bytes, Endian::Big)?;
        if raw.version != 3 {
            return Err(Error::UnsupportedVersion {
                format: "EXH",
                version: raw.version,
            });
        }
        let variant = match raw.variant {
            1 => Variant::Default,
            2 => Variant::Subrows,
            v => return Err(Error::UnsupportedVariant(v)),
        };
        // Check every table span before allocating metadata from its counts.
        let columns_end = table_end(bytes, 32, usize::from(raw.columns), 4)?;
        let pages_end = table_end(bytes, columns_end, usize::from(raw.pages), 8)?;
        let languages_end = table_end(bytes, pages_end, usize::from(raw.languages), 2)?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(usize::from(raw.columns))
            .map_err(|_| Error::AllocationFailed)?;
        for record in bytes[32..columns_end].as_chunks::<4>().0 {
            let column: ColumnRaw = read(record, Endian::Big)?;
            let data_type = ColumnType::from_id(column.kind)?;
            if usize::from(column.offset) + data_type.width() > usize::from(raw.fixed_size) {
                return Err(Error::InvalidData("column exceeds fixed row size"));
            }
            columns.push(Column {
                data_type,
                offset: column.offset,
            });
        }
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(usize::from(raw.pages))
            .map_err(|_| Error::AllocationFailed)?;
        for record in bytes[columns_end..pages_end].as_chunks::<8>().0 {
            let page: PageRaw = read(record, Endian::Big)?;
            if u64::from(page.start_id) + u64::from(page.row_count) > u64::from(u32::MAX) + 1 {
                return Err(Error::InvalidData("page range exceeds row ID space"));
            }
            pages.push(Page {
                start_id: page.start_id,
                row_count: page.row_count,
            });
        }
        pages.sort_unstable_by_key(|p| p.start_id);
        for pair in pages.windows(2) {
            if pair[0].start_id == pair[1].start_id
                || u64::from(pair[0].start_id) + u64::from(pair[0].row_count)
                    > u64::from(pair[1].start_id)
            {
                return Err(Error::InvalidData("duplicate or overlapping page ranges"));
            }
        }
        let mut languages = Vec::new();
        languages
            .try_reserve_exact(usize::from(raw.languages))
            .map_err(|_| Error::AllocationFailed)?;
        for record in bytes[pages_end..languages_end].as_chunks::<2>().0 {
            // Unlike the header/column/page fields, language IDs are little-endian.
            let language = Language::from_id(read(record, Endian::Little)?)?;
            if languages.contains(&language) {
                return Err(Error::InvalidData("duplicate language"));
            }
            languages.push(language);
        }
        Ok(Self {
            fixed_size: raw.fixed_size,
            variant,
            row_count: raw.row_count,
            columns,
            pages,
            languages,
        })
    }

    pub fn fixed_row_size(&self) -> u16 {
        self.fixed_size
    }
    pub fn variant(&self) -> Variant {
        self.variant
    }
    /// The count declared by EXH, not independently verified against all pages.
    pub fn row_count(&self) -> u32 {
        self.row_count
    }
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Pages sorted by start ID. Gaps between ranges are permitted.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    pub fn page_for_row(&self, id: u32) -> Option<&Page> {
        let i = self.pages.partition_point(|page| page.start_id <= id);
        self.pages
            .get(i.checked_sub(1)?)
            .filter(|page| page.contains(id))
    }

    /// Generate page paths for a sheet name relative to `exd/`, without extension.
    /// An empty language list denotes unlocalized data and supports `None` only.
    pub fn page_paths(
        &self,
        sheet: &str,
        language: Language,
    ) -> Result<impl ExactSizeIterator<Item = String> + '_> {
        if !sheet.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        }) {
            return Err(Error::InvalidSheetName);
        }
        if !(self.languages.contains(&language)
            || self.languages.is_empty() && language == Language::None)
        {
            return Err(Error::LanguageUnavailable(language));
        }
        let sheet = sheet.to_ascii_lowercase();
        Ok(self
            .pages
            .iter()
            .map(move |page| format!("exd/{sheet}_{}{}.exd", page.start_id, language.suffix())))
    }
}
