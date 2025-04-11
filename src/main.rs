use std::{fs, path::{Path, PathBuf}, u128, u64, usize};
use anyhow::{anyhow, Context};
use binrw::{prelude::*, Endian::Big};
use clap::{command, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use leb128;
use hex_literal::hex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use std::io::Cursor;

use binrw::{Endian, io::{Write,Seek, Read, SeekFrom},helpers::until_eof};

#[derive(Debug, Clone,Copy)]
struct LEB128(u64);

impl From<LEB128> for usize {
    fn from(value: LEB128) -> Self {
        value.0 as usize
    }
}
impl TryFrom<usize> for LEB128 {
    type Error = std::io::Error;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value > u64::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Value too large for LEB128",
            ));
        }
        Ok(LEB128(value as u64))
    }
}

impl BinRead for LEB128 {
    type Args<'a> = ();
    fn read_options<R: std::io::Read + std::io::Seek>(
            reader: &mut R,
            endian: binrw::Endian,
            args: Self::Args<'_>,
        ) -> BinResult<Self> {
        match leb128::read::unsigned(reader) {
            Ok(value)=> Ok(LEB128(value)),
            Err(e) => return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(e),
            }),
        }
    }
}

impl BinWrite for LEB128 {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + std::io::Seek>(
            &self,
            writer: &mut W,
            _: binrw::Endian,
            _: Self::Args<'_>,
        ) -> BinResult<()> {
        match leb128::write::unsigned(writer, self.0) {
            Ok(_) => Ok(()),
            Err(e) => return Err(binrw::Error::Custom {
                pos: writer.stream_position().unwrap_or(0),
                err: Box::new(e),
            }),
        }
    }
}

#[binrw]
#[derive(Debug, Clone,Default,Serialize,Deserialize)]
pub struct SizedString(
    #[br(parse_with = parse_sized_string)]
    #[bw(write_with = write_sized_string)]
    String
);


fn parse_sized_string<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    _args: (),
) -> BinResult<String> {
    // Read LEB128 length
    let length = LEB128::read_options(reader, endian, ())?;
    
    // Convert to usize and create proper VecArgs
    let usize_length: usize = length.into();
    let bytes = Vec::<u8>::read_args(
        reader, 
        binrw::args! { count: usize_length }
    )?;
    
    String::from_utf8(bytes).map_err(|e| binrw::Error::Custom {
        pos: reader.stream_position().unwrap_or(0),
        err: Box::new(e),
    })
}

fn write_sized_string<W: std::io::Write + std::io::Seek>(
    s: &String,
    writer: &mut W,
    endian: binrw::Endian,  // Now using the endian parameter
    _args: (),
) -> BinResult<()> {
    let length = LEB128(s.len() as u64);
    // Use write_options with explicit endian handling
    length.write_options(writer, endian, ())?;
    writer.write_all(s.as_bytes())?;
    Ok(())
}

#[derive(BinRead, BinWrite, Debug, Clone,Serialize, Deserialize)]
pub struct Event {
    pub event_type: u32,
    pub nest_depth: u32,
    pub event_data: EventData,
   
}

#[derive(BinRead, BinWrite, Debug, Clone,Serialize, Deserialize)]
pub struct EventData {
    #[br(parse_with = read_until_null)]
    #[serde(serialize_with = "serialize_bytes_as_hex",
            deserialize_with = "deserialize_bytes_from_hex")]
    pub data: Vec<u8>,
    seperator: u8,
    #[br(parse_with = parse_variables, args(data.clone()))]
    pub variables: Vec<EventDataType>,
}


#[derive(BinRead, BinWrite, Debug, Clone,Serialize,Deserialize)]
#[br(import(code: u8))]
#[serde(tag = "type", content = "value")]
pub enum EventDataType {
    #[br(pre_assert(code == 0x01))]
    U32(u32),
    
    #[br(pre_assert(code == 0x02))]
    
    U128(#[serde(serialize_with = "serialize_u128_as_hex", deserialize_with = "deserialize_u128_from_hex")] u128),
    
    #[br(pre_assert(code == 0x03))]
    Text(SizedString),
    
    #[br(pre_assert(code == 0x04))]
    VariableName(SizedString),
    
    #[br(pre_assert(code == 0x05))]
    SwitchName(SizedString),

    #[br(pre_assert(code == 0x06))]
    Position {
        #[serde(serialize_with = "serialize_u128_as_hex",
            deserialize_with = "deserialize_u128_from_hex")]
        value: u128,
        data: u32,//TODO: tobe determined if it's a u32 + u8 or u32 + SizedString
        data2: u8,
        x: f32,
        y: f32,
        z: f32,
    },
    
    #[br(pre_assert(code == 0x07))]
    Array {
        array_name:SizedString,
        array_type:u32,

        #[brw(if(0x01 == array_type.clone()))]
        value1: u32,
        #[brw(if(0x02 == array_type.clone()))]
        #[serde(serialize_with = "serialize_u128_as_hex",
            deserialize_with = "deserialize_u128_from_hex")]
        value2:u128,
        #[brw(if(0x03 == array_type.clone()))]
        value3: SizedString,
        #[brw(if(0x04 == array_type.clone()))]
        value4: SizedString,
        #[brw(if(0x05 == array_type.clone()))]
        value5: SizedString,

        #[brw(if(0xFF == array_type.clone()))]
        value: u32,

    },
    
    #[br(pre_assert(code == 0x08))]
    Float(f32),
}

// Custom parser for null-terminated data section
fn read_until_null<R: Read + Seek>(
    reader: &mut R,
    _endian: Endian,
    _args: (),
) -> BinResult<Vec<u8>> {
    let mut data = Vec::new();
    loop {
        let byte = u8::read(reader)?;
        if byte == 0 {
            reader.seek(SeekFrom::Current(-1))?;
            break;
        }
        data.push(byte);
    }
    Ok(data)
}

// Custom parser for variables
fn parse_variables<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    (codes,): (Vec<u8>,),
) -> BinResult<Vec<EventDataType>> {
    codes.into_iter()
        .map(|code| EventDataType::read_options(reader, endian, (code,)))
        .collect()
}


#[derive(Debug,Clone,Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Section{
    //section_length: u32,
    section_type: u16,

    #[serde(serialize_with = "serialize_u128_as_hex",
            deserialize_with = "deserialize_u128_from_hex")]
    data: u128,
    section_data: SectionData,

    #[serde(serialize_with = "serialize_bytes_as_hex",
            deserialize_with = "deserialize_bytes_from_hex")]
    unparsed_bytes: Vec<u8>,
}

impl BinRead for Section {
    type Args<'a> = ();
    
    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let section_length = u32::read_options(reader, endian, ())?;
        let section_type = u16::read_options(reader, Big, ())?;

        let start_position:u64 = reader.stream_position().unwrap();
        let data = u128::read_options(reader, endian, ())?;
        // let test_text = SizedString::read_options(reader, endian, ())?;
        let section_data = parse_section(reader, endian, (section_type,))?;
        let end_position:u64 = reader.stream_position().unwrap();

        let mut unparsed_bytes = vec![0; (section_length as u64 - (end_position - start_position)) as usize];
        reader.read_exact(&mut unparsed_bytes)?;

        /*println!("unparsed bytes:");
        for byte in unparsed_bytes.iter() {
            print!("{:02x} ", byte);
        }*/


        Ok(Section {
            //section_length,
            section_type,
            data,
            // test_text,
            section_data,
            unparsed_bytes,
        })
    }
}

impl BinWrite for Section {
    type Args<'a> = ();
    
    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<()> {
        //self.section_length.write_options(writer, endian, ())?;
        0u32.write_options(writer, endian, ())?;

        self.section_type.write_options(writer, Big, ())?;

        let start_position:u64 = writer.stream_position().unwrap();
        //println!("Start position: {}", start_position);

        self.data.write_options(writer, endian, ())?;
        // self.test_text.write_options(writer, endian, ())?;
        self.section_data.write_options(writer, endian, ())?;
        self.unparsed_bytes.write_options(writer, endian, ())?;
        
        let end_position:u64 = writer.stream_position().unwrap();
        // println!("End position: {}", end_position);

        let section_length = (end_position - start_position) as u32;
        writer.seek(SeekFrom::Start(start_position - 6))?;
        section_length.write_options(writer, endian, ())?;
        writer.seek(SeekFrom::Start(end_position))?;

        Ok(())
    }
}

fn parse_section<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    (code,): (u16,),
) -> BinResult<SectionData> {
    SectionData::read_options(reader, endian, (code,))      
}


#[binrw]
#[derive(Debug, Clone,Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[br(import(code: u16))]
enum SectionData {
    #[br(pre_assert(code == 0x1007))]
    EventSheet{
        name: SizedString,
        padding: u16,
        entity_eventsheet_count: u32,//not sure why it's here
        #[bw(try_calc(u32::try_from(events.len())))]
        event_count: u32,
        #[br(count = event_count)]
        events: Vec<Event>,
        eventsheet_section_end: u16,
    },

    #[br(pre_assert(code == 0x0007))]
    EntityHeader{
        object_name: SizedString,
        data: u16,
        eventsheet_condition_count: u32,
        //TODO: not nessary if we only care about text;

    },

    #[br(pre_assert(code == 0x000C))]
    ItemData{
        name: SizedString,
        note: SizedString,
        #[serde(serialize_with = "serialize_u128_as_hex",
            deserialize_with = "deserialize_u128_from_hex")]
        data: u128,
        data2: u8,
        description: SizedString,
        data3: u32
    },

    #[br(pre_assert(true))]
    Unknown{}
}

fn serialize_bytes_as_hex<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let hex_string = hex::encode(bytes);
    serializer.serialize_str(&hex_string)
}

fn deserialize_bytes_from_hex<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let hex_string = String::deserialize(deserializer)?;
    hex::decode(&hex_string).map_err(serde::de::Error::custom)
}



fn serialize_u128_as_hex<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let hex_string = format!("{:032x}", value);
    serializer.serialize_str(&hex_string)
}

fn deserialize_u128_from_hex<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let hex_string = String::deserialize(deserializer)?;
    u128::from_str_radix(&hex_string, 16).map_err(serde::de::Error::custom)
}



#[binrw]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RbrFile {
    #[br(parse_with = validate_header)]
    header: RbrHeader,
    #[br(parse_with = until_eof)]
    sections: Vec<Section>,
}


#[binrw]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[brw(magic = b"YUKAR")]
struct RbrHeader {
    header_length: u32,
    data: u16,
    #[br(count = header_length)]
    data2: Vec<u8>,
}

fn validate_header<R: Read + Seek>(reader: &mut R, endian: Endian, _: ()) -> BinResult<RbrHeader> {
    let header = RbrHeader::read_options(reader,endian,());
    match header {
        Ok(_header) => Ok(_header),
        Err(_) => return Err(binrw::Error::Custom {
            pos: reader.stream_position().unwrap_or(0),
            err: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid RBR file header",
            )),
        })
    }
    
}




#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

}

#[derive(Subcommand)]
enum Commands {
    /// Convert binary RBR files to JSON
    Parse {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        output: PathBuf,

        #[arg(long, help = "Clear output directory before processing")]
        clean: bool,
    },
    /// Convert JSON files back to binary RBR format
    Encode {
        #[arg(short, long)]
        input: PathBuf,
        
        #[arg(short, long)]
        output: PathBuf,

        #[arg(long, help = "Clear output directory before processing")]
        clean: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    match &cli.command {
        Commands::Parse{input,output,clean} => process_files(
            &input,
            &output,
            "rbr",
            "json",
            parse_rbr_file,
            *clean
        )?,
        Commands::Encode{input,output,clean} => process_files(
            &input,
            &output,
            "json",
            "rbr",
            encode_json_to_rbr,
            *clean
        )?,
    }
    
    Ok(())
}


fn process_files(
    input_dir: &Path,
    output_dir: &Path,
    input_ext: &str,
    output_ext: &str,
    processor: impl Fn(&Path, &Path) -> anyhow::Result<()>,
    clean: bool,
) -> anyhow::Result<()> {

    

    if clean{
        let output_abs = fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf());
        let input_abs = fs::canonicalize(input_dir).unwrap_or_else(|_| input_dir.to_path_buf());
        if output_abs == input_abs {
            return Err(anyhow!(
                "Safety error: Input and output directories are identical ({})",
                output_dir.display()
            ));
        }
        
        if output_dir.exists() {
            println!("Cleaning output directory: {}", output_dir.display());
            fs::remove_dir_all(output_dir)
            .context("Failed to clean output directory")?;
            fs::create_dir_all(output_dir)
            .context("Failed to recreate output directory")?;
        }
    }

    let files: Vec<PathBuf> = WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == input_ext))
        .map(|e| e.into_path())
        .collect();

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
        .unwrap()
        .progress_chars("#>-"));
    let mut errors = 0;

    for file in &files {
        let relative_path = file.strip_prefix(input_dir)?;
        let output_path = output_dir.join(relative_path).with_extension(output_ext);
        
        // Update progress bar with current file name
        pb.set_message(
            file.display()
                .to_string()
                .chars()
                .take(90)
                .collect::<String>(),
        );
        
        // Create output directory if it doesn't exist
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed creating directory for {}", file.display()))?;
        }

        // Process file with error handling
        match processor(file, &output_path) {
            Ok(_) => pb.inc(1),
            Err(e) => {
                pb.println(format!("❌ Error processing {}: {}", file.display(), e));
                pb.inc(1);
                errors += 1;
            }
        }

    }

    pb.finish_with_message(format!(
        "Completed processing {} files. with {} errors.",
        files.len(),errors
    ));
    Ok(())
}


fn parse_rbr_file(input_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    // Read binary file
    let data = fs::read(input_path)?;
    let mut cursor = Cursor::new(data);
    
    // Parse using your existing BinRead implementation
    let file = RbrFile::read_options(&mut cursor,Endian::Little,())?;
    
    // Serialize to JSON
    let json = serde_json::to_string_pretty(&file)?;
    
    // Write JSON file
    fs::write(output_path, json)?;
    
    Ok(())
}

/// Encode JSON file back to binary RBR format
fn encode_json_to_rbr(input_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    // Read JSON file
    let json = fs::read_to_string(input_path)?;
    
    // Deserialize from JSON
    let file: RbrFile = serde_json::from_str(&json)?;
    
    // Write binary file
    let mut cursor = Cursor::new(Vec::new());
    file.write_options(&mut cursor,Endian::Little,())?;
    fs::write(output_path, cursor.into_inner())?;
    
    Ok(())
}