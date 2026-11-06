"""Independent Win32 SGB1/SCN1 furniture pointer observations, using struct.

Only structural type-1 BG and type-6 SharedGroup asset pointers are dependency
evidence. No arbitrary string scanning or execution of scene content.
"""
import struct


def observe(b):
    assert len(b)<=64<<20 and len(b)>=84
    assert struct.unpack_from('<4sII',b)==(b'SGB1',len(b),1)
    tag,size,offset,count=struct.unpack_from('<4sIII',b,12)
    assert tag==b'SCN1' and size==72 and count==1
    work=0
    def read(fmt,at):
        nonlocal work
        work+=1;assert work<1_000_000
        assert 0<=at<=len(b)-struct.calcsize(fmt)
        return struct.unpack_from(fmt,b,at)
    def pointer(origin,field):
        relative,=read('<i',field);at=origin+relative;assert relative>=0 and 0<=at<len(b);return at
    def string(origin,field):
        relative,=read('<I',field);at=origin+relative;assert 0<=at<len(b);end=b.index(b'\0',at);assert end-at<=1024*1024;return b[at:end]
    group=20+offset;gid,=read('<I',group);name=string(group,group+4);table=pointer(group,group+8);n,=read('<I',group+12);assert n<=65536
    layers=[]
    for i in range(n):
        layer=pointer(table,table+i*4);identity,=read('<I',layer);layer_name=string(layer,layer+4)
        objects=pointer(layer,layer+8);m,=read('<I',layer+12);assert m<=65536
        instances=[]
        for j in range(m):
            instance=pointer(objects,objects+j*4);typ,instance_id=read('<II',instance);iname=string(instance,instance+8);trs=read('<9f',instance+12)
            paths=[]
            if typ in [1,6]:
                paths.append(('model'if typ==1 else 'shared_group',string(instance,instance+48)))
                if typ==1:paths.append(('collision',string(instance,instance+52)))
            instances.append({'offset':instance,'type':typ,'id':instance_id,'name':iname,'trs':trs,'paths':paths})
        layers.append({'offset':layer,'id':identity,'name':layer_name,'instances':instances})
    return {'group_offset':group,'group_id':gid,'group_name':name,'layers':layers}


if __name__=='__main__':
    from pathlib import Path
    import sys
    for path in sys.argv[1:]:print(path,observe(Path(path).read_bytes()))
