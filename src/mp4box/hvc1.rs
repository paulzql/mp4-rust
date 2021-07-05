use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Seek, Write};
use serde::{Serialize};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hvc1Box {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,

    #[serde(with = "value_u32")]
    pub horizresolution: FixedPointU16,

    #[serde(with = "value_u32")]
    pub vertresolution: FixedPointU16,
    pub frame_count: u16,
    pub depth: u16,
    pub hvcc: HvcCBox,
}

impl Default for Hvc1Box {
    fn default() -> Self {
        Hvc1Box {
            data_reference_index: 0,
            width: 0,
            height: 0,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            hvcc: HvcCBox::default(),
        }
    }
}

impl Hvc1Box {
    pub fn new(config: &HvcConfig) -> Self {
        Hvc1Box {
            data_reference_index: 1,
            width: config.width,
            height: config.height,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            hvcc: HvcCBox::new(config.video_param_sets.iter().map(|v| v.as_slice()).collect(),
            config.seq_param_sets.iter().map(|v| v.as_slice()).collect(),
            config.pic_param_sets.iter().map(|v| v.as_slice()).collect(),
            config.supplementary_enhancement_information.iter().map(|v| v.as_slice()).collect()),
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::Hvc1Box
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 8 + 70 + self.hvcc.box_size()
    }
}

impl Mp4Box for Hvc1Box {
    fn box_type(&self) -> BoxType {
        return self.get_type();
    }

    fn box_size(&self) -> u64 {
        return self.get_size();
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!("data_reference_index={} width={} height={} frame_count={}",
            self.data_reference_index, self.width, self.height, self.frame_count);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Hvc1Box {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        reader.read_u64::<BigEndian>()?; // pre-defined
        reader.read_u32::<BigEndian>()?; // pre-defined
        let width = reader.read_u16::<BigEndian>()?;
        let height = reader.read_u16::<BigEndian>()?;
        let horizresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        let vertresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        reader.read_u32::<BigEndian>()?; // reserved
        let frame_count = reader.read_u16::<BigEndian>()?;
        skip_bytes(reader, 32)?; // compressorname
        let depth = reader.read_u16::<BigEndian>()?;
        reader.read_i16::<BigEndian>()?; // pre-defined

        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if name == BoxType::HvcCBox {
            let hvcc = HvcCBox::read_box(reader, s)?;

            skip_bytes_to(reader, start + size)?;

            Ok(Hvc1Box {
                data_reference_index,
                width,
                height,
                horizresolution,
                vertresolution,
                frame_count,
                depth,
                hvcc,
            })
        } else {
            Err(Error::InvalidData("hvcc not found"))
        }
    }
}

impl<W: Write> WriteBox<&mut W> for Hvc1Box {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.data_reference_index)?;

        writer.write_u32::<BigEndian>(0)?; // pre-defined, reserved
        writer.write_u64::<BigEndian>(0)?; // pre-defined
        writer.write_u32::<BigEndian>(0)?; // pre-defined
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        writer.write_u32::<BigEndian>(self.horizresolution.raw_value())?;
        writer.write_u32::<BigEndian>(self.vertresolution.raw_value())?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.frame_count)?;
        // skip compressorname
        write_zeros(writer, 32)?;
        writer.write_u16::<BigEndian>(self.depth)?;
        writer.write_i16::<BigEndian>(-1)?; // pre-defined

        self.hvcc.write_box(writer)?;

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct HvcCBox {
    pub general_configuration: [u8; 12],
    pub num_temporal_layer: u8,
    pub chroma_idc: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub temporal_id_nested: bool,
    pub video_parameter_sets: Vec<NalUnit>,
    pub sequence_parameter_sets: Vec<NalUnit>,
    pub picture_parameter_sets: Vec<NalUnit>,
    pub supplementary_enhancement_information: Vec<NalUnit>,
}

impl HvcCBox {
    pub fn new(vps: Vec<&[u8]>, sps: Vec<&[u8]>, pps: Vec<&[u8]>, sei: Vec<&[u8]>) -> Self {
        Self {
            general_configuration: [0; 12],
            num_temporal_layer: 0,
            chroma_idc: 0,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            temporal_id_nested: false,
            video_parameter_sets: vps.into_iter().map(|v| NalUnit::from(v)).collect(),
            sequence_parameter_sets: sps.into_iter().map(|v| NalUnit::from(v)).collect(),
            picture_parameter_sets: pps.into_iter().map(|v| NalUnit::from(v)).collect(),
            supplementary_enhancement_information: sei.into_iter().map(|v| NalUnit::from(v)).collect()
        }
    }
}

impl Mp4Box for HvcCBox {
    fn box_type(&self) -> BoxType {
        BoxType::HvcCBox
    }

    fn box_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 23;
        if self.video_parameter_sets.len() > 0 {
            size += 3;
            for vps in self.video_parameter_sets.iter() {
                size += vps.size() as u64;
            }
        }
        if self.sequence_parameter_sets.len() > 0 {
            size += 3;
            for sps in self.sequence_parameter_sets.iter() {
                size += sps.size() as u64;
            }
        }
        if self.picture_parameter_sets.len() > 0 {
            size += 3;
            for pps in self.picture_parameter_sets.iter() {
                size += pps.size() as u64;
            }
        }
        if self.supplementary_enhancement_information.len() > 0 {
            size += 3;
            for sei in self.supplementary_enhancement_information.iter() {
                size += sei.size() as u64;
            }
        }
        size
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!("chroma_idc={}",
            self.chroma_idc);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for HvcCBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;
        reader.read_u8()?; // 0x01
        let mut general_configuration = [0u8; 12];
        reader.read_exact(&mut general_configuration[..])?;
        reader.read_u16::<BigEndian>()?; //0xF000 min spatial segmentation
        reader.read_u8()?; // 0xFC parallelism type since segmentation
        let chroma_idc = reader.read_u8()? & 0x03;
        let bit_depth_luma_minus8 = reader.read_u8()? & 0x07;
        let bit_depth_chroma_minus8 = reader.read_u8()? & 0x07;
        reader.read_i16::<BigEndian>()?; // 0x0000 framerate
        let stc = reader.read_u8()?;
        let num_temporal_layer = stc >> 3;
        let temporal_id_nested = (stc & 0x04) == 0x04;
        let num_nals = reader.read_u8()?;
        let mut video_parameter_sets = Vec::new();
        let mut sequence_parameter_sets = Vec::new();
        let mut picture_parameter_sets = Vec::new();
        let mut supplementary_enhancement_information = Vec::new();

        let mut i_nal = 0;
        while i_nal < num_nals {
            let sub_nal_type = reader.read_u8()?;
            let sub_nal_num = reader.read_u16::<BigEndian>()?;
            for _ in 0..sub_nal_num {
                let nal_unit = NalUnit::read(reader)?;
                match sub_nal_type {
                    32 => video_parameter_sets.push(nal_unit),
                    33 => sequence_parameter_sets.push(nal_unit),
                    34 => picture_parameter_sets.push(nal_unit),
                    39 => supplementary_enhancement_information.push(nal_unit),
                    _ => ()
                }
                i_nal += 1;
            }
        }

        skip_bytes_to(reader, start + size)?;

        Ok(HvcCBox {
            general_configuration,
            chroma_idc,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            num_temporal_layer,
            temporal_id_nested,
            video_parameter_sets,
            sequence_parameter_sets,
            picture_parameter_sets,
            supplementary_enhancement_information
        })
    }
}

impl<W: Write> WriteBox<&mut W> for HvcCBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;
        writer.write_u8(0x01)?;
        writer.write_u16::<BigEndian>(0xF000)?;
        writer.write_u8(0xFC)?;
        writer.write_u8(0xFC | (self.chroma_idc & 0x03))?;
        writer.write_u8(0xF8 | (self.bit_depth_luma_minus8 & 0x07))?;
        writer.write_u8(0xF8 | (self.bit_depth_chroma_minus8 & 0x07))?;
        writer.write_u16::<BigEndian>(0x0000)?; // framerate
        let temporal_id_nested = if self.temporal_id_nested {1} else {0};
        writer.write_u8(((self.num_temporal_layer & 0x07) << 3) | ((temporal_id_nested << 2) | 0x03))?;
        writer.write_u8((self.video_parameter_sets.len() + self.sequence_parameter_sets.len()
            + self.picture_parameter_sets.len() + self.supplementary_enhancement_information.len()) as u8)?;
        if self.video_parameter_sets.len() > 0 {
            writer.write_u8(32)?;
            writer.write_u16::<BigEndian>(self.video_parameter_sets.len() as u16)?;
            for vps in self.video_parameter_sets.iter() {
                vps.write(writer)?;
            }
        }
        if self.sequence_parameter_sets.len() > 0 {
            writer.write_u8(33)?;
            writer.write_u16::<BigEndian>(self.sequence_parameter_sets.len() as u16)?;
            for sps in self.sequence_parameter_sets.iter() {
                sps.write(writer)?;
            }
        }
        if self.picture_parameter_sets.len() > 0 {
            writer.write_u8(34)?;
            writer.write_u16::<BigEndian>(self.picture_parameter_sets.len() as u16)?;
            for pps in self.picture_parameter_sets.iter() {
                pps.write(writer)?;
            }
        }
        if self.supplementary_enhancement_information.len() > 0 {
            writer.write_u8(39)?;
            writer.write_u16::<BigEndian>(self.supplementary_enhancement_information.len() as u16)?;
            for sei in self.supplementary_enhancement_information.iter() {
                sei.write(writer)?;
            }
        }
        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct NalUnit {
    pub bytes: Vec<u8>,
}

impl From<&[u8]> for NalUnit {
    fn from(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }
}

impl NalUnit {
    fn size(&self) -> usize {
        2 + self.bytes.len()
    }

    fn read<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u16::<BigEndian>()? as usize;
        let mut bytes = vec![0u8; length];
        reader.read(&mut bytes)?;
        Ok(NalUnit { bytes })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<u64> {
        writer.write_u16::<BigEndian>(self.bytes.len() as u16)?;
        writer.write(&self.bytes)?;
        Ok(self.size() as u64)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::mp4box::BoxHeader;
//     use std::io::Cursor;
//
//     #[test]
    // fn test_avc1() {
    //     let src_box = Avc1Box {
    //         data_reference_index: 1,
    //         width: 320,
    //         height: 240,
    //         horizresolution: FixedPointU16::new(0x48),
    //         vertresolution: FixedPointU16::new(0x48),
    //         frame_count: 1,
    //         depth: 24,
    //         avcc: AvcCBox {
    //             configuration_version: 1,
    //             avc_profile_indication: 100,
    //             profile_compatibility: 0,
    //             avc_level_indication: 13,
    //             length_size_minus_one: 3,
    //             sequence_parameter_sets: vec![NalUnit {
    //                 bytes: vec![
    //                     0x67, 0x64, 0x00, 0x0D, 0xAC, 0xD9, 0x41, 0x41, 0xFA, 0x10, 0x00, 0x00,
    //                     0x03, 0x00, 0x10, 0x00, 0x00, 0x03, 0x03, 0x20, 0xF1, 0x42, 0x99, 0x60,
    //                 ],
    //             }],
    //             picture_parameter_sets: vec![NalUnit {
    //                 bytes: vec![0x68, 0xEB, 0xE3, 0xCB, 0x22, 0xC0],
    //             }],
    //         },
    //     };
    //     let mut buf = Vec::new();
    //     src_box.write_box(&mut buf).unwrap();
    //     assert_eq!(buf.len(), src_box.box_size() as usize);
    //
    //     let mut reader = Cursor::new(&buf);
    //     let header = BoxHeader::read(&mut reader).unwrap();
    //     assert_eq!(header.name, BoxType::Avc1Box);
    //     assert_eq!(src_box.box_size(), header.size);
    //
    //     let dst_box = Avc1Box::read_box(&mut reader, header.size).unwrap();
    //     assert_eq!(src_box, dst_box);
    // }
// }
