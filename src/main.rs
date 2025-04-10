use std::{f32::consts::E, fmt::Error, u128, u64, usize};
use binrw::{prelude::*, Endian::Big, NullString};
use leb128;
use hex_literal::hex;
use std::io::Cursor;

use binrw::{prelude::*, Endian, io::{Write,Seek, Read, SeekFrom}};

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
#[derive(Debug, Clone,Default)]
pub struct SizedString(
    #[br(parse_with = parse_sized_string)]
    #[bw(write_with = write_sized_string)]
    String
);

trait SetValue {
    fn set_value(&mut self, value: String);
}

impl SetValue for SizedString {
    fn set_value(&mut self, value: String) {
        self.0 = value;
    }
}

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

#[derive(BinRead, BinWrite, Debug, Clone)]
pub struct Event {
    pub event_type: u32,
    pub nest_depth: u32,
    pub event_data: EventData,
   
}

#[derive(BinRead, BinWrite, Debug, Clone)]
pub struct EventData {
    #[br(parse_with = read_until_null)]
    pub data: Vec<u8>,
    seperator: u8,
    #[br(parse_with = parse_variables, args(data.clone()))]
    pub variables: Vec<EventDataType>,
}


#[derive(BinRead, BinWrite, Debug, Clone)]
#[br(import(code: u8))]
pub enum EventDataType {
    #[br(pre_assert(code == 0x01))]
    U32(u32),
    
    #[br(pre_assert(code == 0x02))]
    U128(u128),
    
    #[br(pre_assert(code == 0x03))]
    Text(SizedString),
    
    #[br(pre_assert(code == 0x04))]
    VariableName(SizedString),
    
    #[br(pre_assert(code == 0x05))]
    SwitchName(SizedString),

    #[br(pre_assert(code == 0x06))]
    Position {
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


#[derive(Debug,Clone)]
struct Section{
    section_length: u32,
    section_type: u16,
    data: u128,
    section_data: SectionData,
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

        println!("unparsed bytes:");
        for byte in unparsed_bytes.iter() {
            print!("{:02x} ", byte);
        }


        Ok(Section {
            section_length,
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
        self.section_length.write_options(writer, endian, ())?;
        self.section_type.write_options(writer, Big, ())?;

        let start_position:u64 = writer.stream_position().unwrap();
        println!("Start position: {}", start_position);

        self.data.write_options(writer, endian, ())?;
        // self.test_text.write_options(writer, endian, ())?;
        self.section_data.write_options(writer, endian, ())?;
        self.unparsed_bytes.write_options(writer, endian, ())?;
        
        let end_position:u64 = writer.stream_position().unwrap();
        println!("End position: {}", end_position);

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
#[derive(Debug, Clone)]
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

    #[br(pre_assert(code == 0x0001))]
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
        data: u128,
        data2: u8,
        description: SizedString,
        data3: u32
    },

    #[br(pre_assert(true))]
    Unknown{}



}



fn main() {
    let item_section_hex = hex!("E2 05 00 00 00 0C 1B 73 43 53 DD 4B 7D 47 B9 10 67 D3 0B 96 54 73 05 49 74 65 6D 73 05 4E 6F 74 65 73 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 44 65 73 63 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 63 00 00 00 00 00 00 00 E9 C6 17 E9 9B 81 67 41 A5 FC C6 F8 5B FB C7 33 CE 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 06 00 00 00 8C 11 BC 82 E0 38 09 43 AA B1 09 A6 70 D5 31 65 00 00 00 00 A7 1E 41 99 3E 2D 70 41 BC EA 3D 83 77 FE 3E F1 00 00 00 00 4F 38 FC 1F 29 55 F8 4D B8 8C 14 34 C6 3F 02 F5 00 00 00 00 2A 50 7F B4 CC 22 B8 46 91 1B C9 5A 0D FE AC A1 00 00 00 00 F2 14 2F 6F E8 26 39 4D 82 70 7D 9A 3B 1A 20 62 00 00 00 00 ED 1D 04 B1 86 BC 6B 45 AA 6D 9E 65 BA E8 35 9C 00 00 00 00 12 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 6D 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 06 00 00 00 01 9C A1 9E AC 82 70 42 90 26 4A 68 72 AA A2 83 00 00 00 00 0D A1 B1 C9 5B 8A 87 4A A4 4B 9A 3E E5 57 04 76 00 00 00 00 36 D9 BA 9B EC 98 7C 4B 92 2A 9A CC 31 FE 62 48 00 00 00 00 32 A9 3B 8E B8 2A 09 48 8B 23 03 B2 D3 84 22 A4 00 00 00 00 98 27 1B CB F8 4A 35 42 8B 33 69 34 88 5F 17 A5 00 00 00 00 19 3A 85 B4 42 01 30 4A B3 1A 27 4C F4 C5 E9 91 00 00 00 00 06 00 00 00 8C 11 BC 82 E0 38 09 43 AA B1 09 A6 70 D5 31 65 00 00 00 00 A7 1E 41 99 3E 2D 70 41 BC EA 3D 83 77 FE 3E F1 00 00 00 00 4F 38 FC 1F 29 55 F8 4D B8 8C 14 34 C6 3F 02 F5 00 00 00 00 2A 50 7F B4 CC 22 B8 46 91 1B C9 5A 0D FE AC A1 00 00 00 00 F2 14 2F 6F E8 26 39 4D 82 70 7D 9A 3B 1A 20 62 00 00 00 00 ED 1D 04 B1 86 BC 6B 45 AA 6D 9E 65 BA E8 35 9C 00 00 00 00 00 00 00 00 84 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 00 42 28 6D 61 78 28 61 2E 61 74 6B 20 2F 20 32 2E 35 20 2D 20 62 2E 64 65 66 20 2F 20 34 2C 20 30 29 20 2B 20 61 2E 65 61 74 6B 20 2A 20 62 2E 65 64 65 66 29 20 2A 20 72 61 6E 64 28 30 2E 39 2C 20 31 29 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 9C A1 9E AC 82 70 42 90 26 4A 68 72 AA A2 83 01 00 00 00 00 00 00 00 01 00 00 00 00 0A 00 00 00 00 00 00 00 00 00 80 3F 00 04 00 00 00 00 00 00 00 01 1B 73 43 53 DD 4B 7D 47 B9 10 67 D3 0B 96 54 73 5E 02 00 00 09 00 00 00 00 00 00 00 19 00 00 00 2E DD EE 8D 03 3E 4B 40 81 4C 42 41 F2 19 A8 0C 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 B2 7A D1 97 D2 AA 86 4B 9C E4 7C 46 C4 E2 5B AF 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 2D 12 63 0F 40 CF DA 47 91 20 F0 9D 46 CF C6 54 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 73 9E 76 69 58 98 60 48 BB E8 45 9E 3F 6F E2 81 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 04 54 C0 83 E9 E9 10 4A 85 A3 D6 C4 40 DB CA FD 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 88 BB EE 1D B0 CD D1 47 85 A8 6B 3B 93 B8 B9 31 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 D8 FE A0 09 BB F7 93 40 85 7E 2E 15 B5 E1 EE 7A 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 5F 20 BD 22 55 C1 3C 4C 84 15 D6 C3 EB 03 C1 7F 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 5F 67 94 A4 F8 ED 59 45 9F DD 37 AA A8 39 23 1D 00 00 00 00 00 00 00 00 01 09 00 00 00 00 00 00 00 19 00 00 00 2E DD EE 8D 03 3E 4B 40 81 4C 42 41 F2 19 A8 0C 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 B2 7A D1 97 D2 AA 86 4B 9C E4 7C 46 C4 E2 5B AF 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 2D 12 63 0F 40 CF DA 47 91 20 F0 9D 46 CF C6 54 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 73 9E 76 69 58 98 60 48 BB E8 45 9E 3F 6F E2 81 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 04 54 C0 83 E9 E9 10 4A 85 A3 D6 C4 40 DB CA FD 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 88 BB EE 1D B0 CD D1 47 85 A8 6B 3B 93 B8 B9 31 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 D8 FE A0 09 BB F7 93 40 85 7E 2E 15 B5 E1 EE 7A 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 5F 20 BD 22 55 C1 3C 4C 84 15 D6 C3 EB 03 C1 7F 00 00 00 00 00 00 00 00 01 00 00 00 00 19 00 00 00 5F 67 94 A4 F8 ED 59 45 9F DD 37 AA A8 39 23 1D 00 00 00 00 00 00 00 00 01 00 00 00 00");
    let event_section_hex = hex!("EB 01 00 00 10 07 36 44 18 E9 8E 83 AC 42 B3 19 91 8B 92 22 BC C6 00 00 00 02 00 00 00 10 00 00 00 1E 00 00 00 00 00 00 00 01 03 03 03 01 04 04 04 00 03 00 00 00 07 43 68 6F 69 63 65 31 07 43 68 6F 69 63 65 32 07 43 68 6F 69 63 65 33 04 00 00 00 00 00 00 1D 00 00 00 01 00 00 00 03 01 01 00 0D 53 65 6C 65 63 74 43 68 6F 69 63 65 31 02 00 00 00 00 00 00 00 1D 00 00 00 01 00 00 00 03 01 01 00 0F 53 65 6C 65 63 74 43 68 6F 69 63 65 31 5F 31 02 00 00 00 00 00 00 00 1E 00 00 00 01 00 00 00 01 03 03 01 04 04 00 02 00 00 00 07 43 68 6F 69 63 65 41 07 43 68 6F 69 63 65 42 04 00 00 00 00 00 1D 00 00 00 02 00 00 00 03 01 01 00 0D 53 65 6C 65 63 74 43 68 6F 69 63 65 41 02 00 00 00 00 00 00 00 4A 00 00 00 01 00 00 00 01 00 01 00 00 00 1D 00 00 00 02 00 00 00 03 01 01 00 0D 53 65 6C 65 63 74 43 68 6F 69 63 65 42 02 00 00 00 00 00 00 00 1D 00 00 00 02 00 00 00 03 01 01 00 0F 53 65 6C 65 63 74 43 68 6F 69 63 65 42 5F 32 02 00 00 00 00 00 00 00 48 00 00 00 01 00 00 00 00 1D 00 00 00 01 00 00 00 03 01 01 00 0F 53 65 6C 65 63 74 43 68 6F 69 63 65 31 5F 32 02 00 00 00 00 00 00 00 4A 00 00 00 00 00 00 00 01 00 01 00 00 00 1D 00 00 00 01 00 00 00 03 01 01 00 0D 53 65 6C 65 63 74 43 68 6F 69 63 65 32 02 00 00 00 00 00 00 00 4A 00 00 00 00 00 00 00 01 00 02 00 00 00 1D 00 00 00 01 00 00 00 03 01 01 00 0D 53 65 6C 65 63 74 43 68 6F 69 63 65 33 02 00 00 00 00 00 00 00 48 00 00 00 00 00 00 00 00 1D 00 00 00 00 00 00 00 03 01 01 00 0A 45 6E 64 4D 65 73 73 61 67 65 02 00 00 00 00 00 00 00 00 00");
    let unknow_section_hex = hex!("21 00 00 00 01 01 36 44 18 E9 8E 83 AC 42 B3 19 91 8B 92 22 BC C6 05 68 65 6C 6C 6F 03 03 03 03 03 03 03 03 00 00 00");
    let mut cursor = Cursor::new(event_section_hex);
    let mut test:Section = cursor.read_le().unwrap();
    println!("String: {:#?}", test);

   /*match test.section_data {
       SectionData::ItemData {ref mut name,ref mut note, data, data2,ref mut description, data3 } => {
        name.set_value("ItemName111111".to_string());
        note.set_value("Note111111".to_string());
        description.set_value("Description111111".to_string());
       },
       _ => {}
   }*/

   



    let mut text2 = Cursor::new(vec![]);
    text2.write_le(&test).unwrap();
    println!("Serialized bytes:");
    for byte in text2.get_ref() {
        print!("{:02x} ", byte);
    }

    /*let mut test2_raw = Cursor::new(text2.into_inner());
    let test2:Section = test2_raw.read_le().unwrap();
    println!("String: {:#?}", test2);*/
}
