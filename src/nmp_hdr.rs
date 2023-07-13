// Copyright © 2023 Vouch.io LLC

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use num;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Deserialize, Serialize, FromPrimitive, PartialEq)]
pub enum NmpOp {
    Read = 0,
    ReadRsp = 1,
    Write = 2,
    WriteRsp = 3,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpErr {
    Ok = 0,
    EUnknown = 1,
    ENoMem = 2,
    EInvalid = 3,
    ETimeout = 4,
    ENoEnt = 5,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Deserialize, Serialize)]
pub enum NmpGroup {
    Default = 0,
    Image = 1,
    Stat = 2,
    Config = 3,
    Log = 4,
    Crash = 5,
    Split = 6,
    Run = 7,
    Fs = 8,
    Shell = 9,
    PerUser = 64,
}

pub trait NmpId {
    fn value(&self) -> u8;
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdDefault {
    Echo = 0,
    ConsEchoCtrl = 1,
    TaskStat = 2,
    MpStat = 3,
    DateTimeStr = 4,
    Reset = 5,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdImage {
    State = 0,
    Upload = 1,
    CoreList = 3,
    CoreLoad = 4,
    Erase = 5,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdStat {
    Read = 0,
    List = 1,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdConfig {
    Val = 0,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdLog {
    Show = 0,
    Clear = 1,
    Append = 2,
    ModuleList = 3,
    LevelList = 4,
    List = 5,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdCrash {
    Trigger = 0,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdRun {
    Test = 0,
    List = 1,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdFs {
    File = 0,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum NmpIdShell {
    Exec = 0,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct NmpHdr {
    pub op: NmpOp,
    pub flags: u8,
    pub len: u16,
    pub group: NmpGroup,
    pub seq: u8,
    pub id: u8,
}

impl NmpHdr {
    pub fn new_req(op: NmpOp, group: NmpGroup, id: u8) -> NmpHdr {
        NmpHdr {
            op,
            flags: 0,
            len: 0,
            group,
            seq: 0,
            id,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, bincode::Error> {
        let mut buffer = Vec::new();
        buffer.write_u8(self.op as u8)?;
        buffer.write_u8(self.flags)?;
        buffer.write_u16::<BigEndian>(self.len)?;
        buffer.write_u16::<BigEndian>(self.group as u16)?;
        buffer.write_u8(self.seq)?;
        buffer.write_u8(self.id)?;
        Ok(buffer)
    }

    pub fn deserialize(cursor: &mut Cursor<&Vec<u8>>) -> Result<NmpHdr, bincode::Error> {
        let op = num::FromPrimitive::from_u8(cursor.read_u8()?).unwrap();
        let flags = cursor.read_u8()?;
        let len = cursor.read_u16::<BigEndian>()?;
        let group = num::FromPrimitive::from_u16(cursor.read_u16::<BigEndian>()?).unwrap();
        let seq = cursor.read_u8()?;
        let id = cursor.read_u8()?;
        Ok(NmpHdr {
            op,
            flags,
            len,
            group,
            seq,
            id,
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct NmpBase {
    pub hdr: NmpHdr,
}

#[derive(Debug, Clone, Copy, PartialEq, FromPrimitive, Deserialize, Serialize)]
pub enum SplitStatus {
    NotApplicable = 0,
    NotMatching = 1,
    Matching = 2,
}

fn default_0() -> i32 {
    0
}

fn default_vec() -> Vec<u8> {
    Vec::new()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageStateEntry {
    #[serde(default = "default_0")]
    pub image: i32,
    pub slot: i32,
    pub version: String,
    #[serde(default = "default_vec")]
    pub hash: Vec<u8>,
    pub bootable: bool,
    pub pending: bool,
    pub confirmed: bool,
    pub active: bool,
    pub permanent: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageStateRsp {
    #[serde(rename = "hdr")]
    pub nmp_base: NmpBase,
    pub rc: i32,
    pub images: Vec<ImageStateEntry>,
    #[serde(rename = "splitStatus")]
    pub split_status: SplitStatus,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageUploadReq {
    #[serde(rename = "data", with = "serde_bytes")]
    pub data: Vec<u8>,
    #[serde(rename = "image")]
    pub image_num: u8,
    #[serde(rename = "len", skip_serializing_if = "Option::is_none")]
    pub len: Option<u32>,
    #[serde(rename = "off", default)]
    pub off: u32,
    #[serde(
        rename = "sha",
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_bytes"
    )]
    pub data_sha: Option<Vec<u8>>,
    #[serde(rename = "upgrade", default, skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<bool>,
}
