"""Bounded type-2 first-block observation, independent of Rust extraction."""
import struct
import zlib


def prefix(f,entry,max_bytes=4<<20):
    f.seek(entry);header=f.read(24)
    if len(header)!=24:return None
    size,kind,target,_,_,count=struct.unpack('<6I',header)
    if kind!=2 or not 1<=target<=max_bytes or not 1<=count<=16384 or not 24+8*count<=size<=2<<20:return None
    offset,span,expected=struct.unpack('<IHH',f.read(8));f.seek(entry+size+offset);hsize,_,stored,expanded=struct.unpack('<4I',f.read(16));n=expanded if stored==32000 else stored
    assert expanded==expected and 16<=hsize<=span and n<=span-hsize
    f.seek(entry+size+offset+hsize);data=f.read(n)
    if stored!=32000:
        d=zlib.decompressobj(-15);data=d.decompress(data,65536);assert d.eof and not d.unused_data and not d.unconsumed_tail
    assert len(data)==expanded
    return data,target
