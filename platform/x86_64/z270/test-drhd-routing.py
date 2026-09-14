#!/usr/bin/env python3
"""Compile actual no_std routing parser; optionally exercise a native snapshot."""
import argparse
import json
from pathlib import Path
import subprocess

repo = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser()
parser.add_argument('--snapshot', type=Path)
args = parser.parse_args()
out = repo/'target/drhd-routing-tests'
out.mkdir(parents=True, exist_ok=True)
source = '''extern crate alloc;
#[path="PARSER"] mod drhd;
fn table(entries: &[u8]) -> Vec<u8> {
 let mut b=vec![0u8;48]; b[..4].copy_from_slice(b"DMAR"); b.extend(entries);
 let len=b.len() as u32; b[4..8].copy_from_slice(&len.to_le_bytes());
 b[9]=0u8.wrapping_sub(b.iter().fold(0u8,|s,v|s.wrapping_add(*v))); b
}
fn unit(base:u64,all:bool,scopes:&[u8])->Vec<u8> {
 let mut b=vec![0u8;16]; b[2..4].copy_from_slice(&(16u16+scopes.len() as u16).to_le_bytes());
 b[4]=all as u8; b[8..16].copy_from_slice(&base.to_le_bytes()); b.extend(scopes); b
}
#[test] fn explicit_scope_beats_fallback_in_any_order() {
 let igd=unit(0xfed90000,false,&[1,8,0,0,0,0,2,0]);
 let pch=unit(0xfed91000,true,&[3,8,0,0,2,0xf0,0x1f,0]);
 for entries in [ [igd.clone(),pch.clone()].concat(), [pch,igd].concat()] {
  let units=drhd::parse(&table(&entries)).unwrap();
  assert_eq!(units[drhd::route(&units,0x10).unwrap()].base,0xfed90000);
  assert_eq!(units[drhd::route(&units,0x17<<3).unwrap()].base,0xfed91000);
 }
}
#[test] fn missing_route_is_not_fallback_to_first() {
 let units=drhd::parse(&table(&unit(0xfed90000,false,&[1,8,0,0,0,0,2,0]))).unwrap();
 assert_eq!(drhd::route(&units,0x100),None);
}
#[test] fn reject_duplicate_routes() {
 let scope=[1,8,0,0,0,0,2,0];
 assert!(drhd::parse(&table(&[unit(0xfed90000,false,&scope),unit(0xfed91000,true,&scope)].concat())).is_err());
 assert!(drhd::parse(&table(&[unit(0xfed90000,true,&[]),unit(0xfed91000,true,&[])].concat())).is_err());
}
#[test] fn reject_malformed_lengths_and_checksum() {
 assert!(drhd::parse(&table(&[0,0,0,0])).is_err());
 assert!(drhd::parse(&table(&unit(0xfed90000,true,&[1,0,0,0,0,0,2,0]))).is_err());
 let mut b=table(&unit(0xfed90000,true,&[])); b[9]^=1;
 assert!(drhd::parse(&b).is_err());
}
#[test] fn reject_unsupported_scopes_and_segments() {
 assert!(drhd::parse(&table(&unit(0xfed90000,false,&[1,10,0,0,0,0,1,0,2,0]))).is_err());
 assert!(drhd::parse(&table(&unit(0xfed90000,false,&[2,8,0,0,0,0,1,0]))).is_err());
 let mut b=unit(0xfed90000,true,&[]); b[6]=1;
 assert!(drhd::parse(&table(&b)).is_err());
}
'''.replace('PARSER', str(repo/'src/device/iommu/intel_vtd/drhd.rs'))
if args.snapshot:
    raw = bytes.fromhex(json.loads(args.snapshot.read_text())['acpi']['DMAR'])
    source += '''
#[test] fn real_z270_igpu_snapshot() {
 let units=drhd::parse(&RAW).unwrap();
 assert_eq!(units.len(),2);
 assert_eq!(units[drhd::route(&units,0x10).unwrap()].base,0xfed90000);
 for bdf in [0xa0,0xb8,0x100,0x400,0x500] {
  assert_eq!(units[drhd::route(&units,bdf).unwrap()].base,0xfed91000);
 }
}
'''.replace('RAW', str(list(raw)))
(out/'test.rs').write_text(source)
subprocess.run(['rustc','--edition=2021','--test',str(out/'test.rs'),'-o',str(out/'test')],check=True)
subprocess.run([str(out/'test')],check=True)
