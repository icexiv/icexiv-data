"""Bounded independent Havok binary v3 wire observation using only struct.

Input is extracted SKLB or a raw tagfile, not a target executable. No third-party
parser is imported. The decoded graph keeps object references as integer IDs.
"""
from pathlib import Path
import struct


class Observer:
    def __init__(self,data,max_array_length=1000000,max_values=2000000):
        assert len(data)<=64*1024*1024
        self.data=data;self.pos=0;self.strings=[b'',None];self.classes=[None];self.objects=[None];self.values=0;self.version=0
        self.max_array_length=max_array_length;self.max_values=max_values

    def take(self,n):
        assert 0<=n<=len(self.data)-self.pos,(self.pos,n)
        b=self.data[self.pos:self.pos+n];self.pos+=n;return b

    def integer(self):
        first=self.take(1)[0];negative=first&1;value=(first>>1)&63;shift=6;byte=first
        while byte&128:
            assert shift<64
            byte=self.take(1)[0];value|=(byte&127)<<shift;shift+=7
        result=-value if negative else value
        assert -0x8000000000000000<=result<=0x7fffffffffffffff
        return result

    def string(self):
        n=self.integer()
        if n<=0:return self.strings[-n]
        assert n<=1024*1024 and len(self.strings)<100000
        s=self.take(n);self.strings.append(s);return s

    def members(self,cls):
        chain=[];seen=set()
        while cls:
            assert cls not in seen
            seen.add(cls);c=self.classes[cls];chain.append(c['members']);cls=c['parent']
        return [m for group in reversed(chain) for m in group]

    def class_id(self,name):
        ids=[i for i,c in enumerate(self.classes) if c and c['name']==name]
        assert len(ids)==1,(name,ids)
        return ids[0]

    def value(self,member,depth):
        assert depth<64
        typ=member['type']
        if typ&16:
            n=self.integer();assert 0<=n<=self.max_array_length
            inner=dict(member,type=typ&~16)
            return self.elements(inner,n,depth+1)
        if typ&32:
            return self.elements(dict(member,type=typ&~32),member['tuple'],depth+1)
        self.values+=1;assert self.values<=self.max_values
        kind=typ&15
        if kind==0:return None
        if kind==1:return self.take(1)[0]
        if kind==2:return self.integer()
        if kind==3:return struct.unpack('<f',self.take(4))[0]
        if 4<=kind<=7:return struct.unpack('<'+str([4,8,12,16][kind-4])+'f',self.take([16,32,48,64][kind-4]))
        if kind==8:return {'ref':self.integer()}
        if kind==9:return self.structs(self.class_id(member['class']),1,depth+1,False)[0]
        if kind==10:return self.string()
        raise ValueError((self.pos,member))

    def elements(self,member,n,depth):
        self.values+=n;assert self.values<=self.max_values and depth<64
        kind=member['type']&15
        if member['type']&48:return [self.value(member,depth+1) for _ in range(n)]
        if kind==9:return self.structs(self.class_id(member['class']),n,depth+1,True)
        if kind==2:
            width=self.integer() if self.version>2 else 4
            assert width in [4,8],(self.pos,width,n)
            return [self.integer() for _ in range(n)]
        if kind==4:
            components=self.integer();assert components in [3,4]
            return [struct.unpack('<'+str(components)+'f',self.take(components*4)) for _ in range(n)]
        return [self.value(member,depth+1) for _ in range(n)]

    def structs(self,cls,n,depth,columnar):
        assert depth<64
        members=self.members(cls);flags=self.take((len(members)+7)//8)
        objects=[{'class':cls,'fields':{}} for _ in range(n)]
        for i,m in enumerate(members):
            if flags[i//8]>>(i%8)&1:
                values=self.elements(m,n,depth+1) if columnar else [self.value(m,depth+1)]
                for obj,v in zip(objects,values):obj['fields'][m['name']]=v
        return objects

    def parse(self,require_end=True):
        assert self.take(8)==bytes.fromhex('1e0db0cacefa11d0')
        assert self.integer()==1
        self.version=self.integer();assert self.version==3
        top=[]
        while True:
            tag=self.integer()
            if tag==7:break
            if tag==2:
                name=self.string();version=self.integer();parent=self.integer();n=self.integer();assert 0<=n<=4096 and len(self.classes)<4096
                fields=[]
                for _ in range(n):
                    field=self.string();typ=self.integer();size=self.integer() if typ&32 else 0;other=self.string() if typ&15 in [8,9] else None
                    fields.append({'name':field,'type':typ,'tuple':size,'class':other})
                self.classes.append({'name':name,'version':version,'parent':parent,'members':fields})
            elif tag in [3,4]:
                cls=self.integer();obj=self.structs(cls,1,0,False)[0]
                if tag==4:self.objects.append(obj)
                top.append(obj)
            elif tag==5:top.append({'ref':self.integer()})
            elif tag==6:top.append(None)
            else:raise ValueError((self.pos,tag))
        if require_end:assert self.pos==len(self.data),(self.pos,len(self.data))
        return top


def read(path,max_array_length=1000000,max_values=2000000):
    data=Path(path).read_bytes()
    pap_end=None
    if data[:4]==b'blks':
        version=data[4:8]
        if version in [b'0031',b'0032']:offset,=struct.unpack_from('<I',data,12)
        elif version==b'0021':offset,=struct.unpack_from('<H',data,10)
        else:raise ValueError(('SKLB version',version))
        data=data[offset:]
    elif data[:4]==b'pap ':
        assert struct.unpack_from('<I',data,4)[0]==0x00020001
        start,end=struct.unpack_from('<II',data,18)
        assert 26<=start<end<=len(data)
        data=data[start:end]
        pap_end=end
    observer=Observer(data,max_array_length,max_values);observer.parse(require_end=pap_end is None)
    if pap_end is not None:
        tail=data[observer.pos:];assert not tail or (len(tail)<=15 and pap_end%16==0 and not any(tail)),(observer.pos,len(data),tail.hex())
    return observer


if __name__=='__main__':
    import sys
    o=read(sys.argv[1]);print('bytes',o.pos,'classes',len(o.classes)-1,'objects',len(o.objects)-1,'values',o.values)
    for i,obj in enumerate(o.objects[1:],1):
        print(i,o.classes[obj['class']]['name'],[(name, len(v) if isinstance(v,list) else repr(v)[:100]) for name,v in obj['fields'].items()])
