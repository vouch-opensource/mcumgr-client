// Copyright © 2023-2024 Vouch.io LLC, 2026 Rudis Laboratories LLC

use hex_buffer_serde::{Hex as _, HexForm};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};
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
#[allow(dead_code)]
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
    fn to_u8(&self) -> u8;
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdDef {
    Echo = 0,
    ConsEchoCtrl = 1,
    TaskStat = 2,
    MpStat = 3,
    DateTimeStr = 4,
    Reset = 5,
    McumgrParams = 6,
    Info = 7,
    BootloaderInfo = 8,
}

impl NmpId for NmpIdDef {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdImage {
    State = 0,
    Upload = 1,
    CoreList = 3,
    CoreLoad = 4,
    Erase = 5,
}

impl NmpId for NmpIdImage {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdStat {
    Read = 0,
    List = 1,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdConfig {
    Val = 0,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum NmpIdCrash {
    Trigger = 0,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdRun {
    Test = 0,
    List = 1,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdFs {
    File = 0,
    FileStat = 1,
    FileHash = 2,
    SupportedHashTypes = 3,
    FileClose = 4,
}

impl NmpId for NmpIdFs {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum NmpIdShell {
    Exec = 0,
}

impl NmpId for NmpIdShell {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
}

impl NmpId for NmpIdStat {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
}

impl NmpId for NmpIdConfig {
    fn to_u8(&self) -> u8 {
        *self as u8
    }
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
    pub fn new_req(op: NmpOp, group: NmpGroup, id: impl NmpId) -> NmpHdr {
        NmpHdr {
            op,
            flags: 0,
            len: 0,
            group,
            seq: 0,
            id: id.to_u8(),
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

fn default_0() -> u32 {
    0
}

fn default_false() -> bool {
    false
}

fn default_vec() -> Vec<u8> {
    Vec::new()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageStateEntry {
    #[serde(default = "default_0")]
    pub image: u32,
    pub slot: u32,
    pub version: String,
    #[serde(default = "default_vec", with = "HexForm")]
    pub hash: Vec<u8>,
    #[serde(default = "default_false")]
    pub bootable: bool,
    #[serde(default = "default_false")]
    pub pending: bool,
    #[serde(default = "default_false")]
    pub confirmed: bool,
    #[serde(default = "default_false")]
    pub active: bool,
    #[serde(default = "default_false")]
    pub permanent: bool,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageStateReq {
    #[serde(with = "serde_bytes")]
    pub hash: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageStateRsp {
    pub images: Vec<ImageStateEntry>,
    #[serde(rename = "splitStatus", skip_serializing_if = "Option::is_none")]
    pub split_status: Option<SplitStatus>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageEraseReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u32>,
}

// OS Management Group Structures

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EchoReq {
    pub d: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EchoRsp {
    pub r: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskStatRsp {
    pub tasks: std::collections::HashMap<String, TaskInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskInfo {
    #[serde(default)]
    pub prio: i32,
    #[serde(default)]
    pub state: u64,
    #[serde(default)]
    pub stkuse: u64,
    #[serde(default)]
    pub stksiz: u64,
    #[serde(default)]
    pub cswcnt: u64,
    #[serde(default)]
    pub runtime: u64,
    #[serde(default)]
    pub last_checkin: u64,
    #[serde(default)]
    pub next_checkin: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McumgrParamsRsp {
    pub buf_size: u32,
    pub buf_count: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsInfoReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsInfoRsp {
    pub output: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BootloaderInfoReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BootloaderInfoRsp {
    #[serde(default)]
    pub bootloader: String,
    #[serde(default)]
    pub mode: Option<i32>,
    #[serde(rename = "no-downgrade", default)]
    pub no_downgrade: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResetReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<u32>,
}

// Shell Management Group Structures

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellExecReq {
    #[serde(rename = "argv")]
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShellExecRsp {
    #[serde(default)]
    pub o: String,
    #[serde(default)]
    pub rc: i32,
}

// File System Management Group Structures

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsDownloadReq {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "off")]
    pub off: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsDownloadRsp {
    #[serde(rename = "off")]
    pub off: u32,
    #[serde(rename = "data", with = "serde_bytes")]
    pub data: Vec<u8>,
    #[serde(rename = "len", skip_serializing_if = "Option::is_none")]
    pub len: Option<u32>,
    #[serde(default)]
    pub rc: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsUploadReq {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "off")]
    pub off: u32,
    #[serde(rename = "data", with = "serde_bytes")]
    pub data: Vec<u8>,
    #[serde(rename = "len", skip_serializing_if = "Option::is_none")]
    pub len: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsUploadRsp {
    #[serde(rename = "off")]
    pub off: u32,
    #[serde(default)]
    pub rc: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsStatReq {
    #[serde(rename = "name")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsStatRsp {
    #[serde(rename = "len")]
    pub len: u32,
    #[serde(default)]
    pub rc: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsHashReq {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub hash_type: Option<String>,
    #[serde(rename = "off", skip_serializing_if = "Option::is_none")]
    pub off: Option<u32>,
    #[serde(rename = "len", skip_serializing_if = "Option::is_none")]
    pub len: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FsHashRsp {
    #[serde(rename = "type")]
    pub hash_type: String,
    #[serde(rename = "off")]
    pub off: u32,
    #[serde(rename = "len")]
    pub len: u32,
    #[serde(rename = "output", with = "serde_bytes")]
    pub output: Vec<u8>,
    #[serde(default)]
    pub rc: i32,
}

// Statistics Management Group Structures

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatListReq {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatListRsp {
    #[serde(default)]
    pub stat_list: Vec<String>,
    #[serde(default)]
    pub rc: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatReadReq {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatReadRsp {
    pub name: String,
    #[serde(default)]
    pub fields: std::collections::HashMap<String, i64>,
    #[serde(default)]
    pub rc: i32,
}

// Settings/Config Management Group Structures

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsReadReq {
    pub name: String,
    #[serde(rename = "max_size", skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsReadRsp {
    #[serde(rename = "val", with = "serde_bytes", default)]
    pub val: Vec<u8>,
    #[serde(default)]
    pub rc: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsWriteReq {
    pub name: String,
    #[serde(rename = "val", with = "serde_bytes")]
    pub val: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsDeleteReq {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsCommitReq {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsLoadReq {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettingsSaveReq {}
