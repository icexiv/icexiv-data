"""Independent spline wire observations and scalar mathematical sampling.

Uses only struct/math; does not import or execute a third-party Havok parser.
The object input comes from observe_tagfile. Results retain channel controls,
making structural boundaries independently inspectable before Rust decoding.
"""
import math
import struct


class Cursor:
    def __init__(self,data,start,end):self.data=data;self.pos=start;self.end=end
    def take(self,n):
        assert 0<=n<=self.end-self.pos,(self.pos,n,self.end)
        b=self.data[self.pos:self.pos+n];self.pos+=n;return b
    def align(self,n):self.take((-self.pos)%n)
    def scalar(self):
        v,=struct.unpack('<f',self.take(4));assert math.isfinite(v);return v
    def shape(self):
        last,degree=struct.unpack('<HB',self.take(3));n=last+1;assert 0<=degree<=8 and n>degree
        knots=list(self.take(n+degree+1));assert knots==sorted(knots) and knots[degree]<knots[n]
        return n,degree,knots


def quaternion(raw,encoding):
    if encoding==5:return struct.unpack('<4f',raw)
    if encoding==0:
        code,=struct.unpack('<I',raw);r=1-((code>>18&1023)/1023)**2
        lattice=code&262143;band=math.isqrt(lattice)
        polar=band*math.pi/(2*511);azimuth=(lattice-band*band)*math.pi/(4*band) if band else 0
        radius=math.sqrt(max(0,1-r*r))
        q=[radius*math.sin(polar)*math.cos(azimuth),radius*math.sin(polar)*math.sin(azimuth),radius*math.cos(polar),r]
        return tuple(-v if code>>(28+i)&1 else v for i,v in enumerate(q))
    if encoding==1:
        code=int.from_bytes(raw,'little');values=[((code>>shift&4095)-2047)/(2047*math.sqrt(2)) for shift in [0,12,24]]
        missing=code>>36&3;negative=code>>38&1
    elif encoding==2:
        words=struct.unpack('<3H',raw);values=[((v&32767)-16383)/(16383*math.sqrt(2)) for v in words]
        missing=(words[0]>>15)|((words[1]>>15)<<1);negative=words[2]>>15
    else:raise ValueError(('unverified quaternion encoding',encoding))
    residual=1-sum(v*v for v in values);assert residual>=0
    values.insert(missing,(-1 if negative else 1)*math.sqrt(residual));return tuple(values)


def vector(c,flags,quantization,identity):
    assert quantization in [0,1]
    states=[1 if flags>>i&1 else (2 if flags>>(i+4)&1 else 0) for i in range(3)]
    assert flags&0x88==0
    shape=c.shape() if 2 in states else None
    if shape:c.align(4)
    channels=[]
    for state in states:
        if state==2:channels.append((c.scalar(),c.scalar()))
        elif state==1:channels.append([c.scalar()])
        else:channels.append([identity])
    if shape:
        n,degree,knots=shape;values=[[] if state==2 else channels[i] for i,state in enumerate(states)]
        for _ in range(n):
            for i,state in enumerate(states):
                if state!=2:continue
                code=int.from_bytes(c.take(1+quantization),'little');low,high=channels[i]
                values[i].append(low+(high-low)*code/((1<<(8*(1+quantization)))-1))
        c.align(4)
        return {'knots':knots,'degree':degree,'channels':values}
    return {'knots':None,'degree':0,'channels':channels}


def rotation(c,flags,encoding):
    widths={0:4,1:5,2:6,5:16};assert encoding in widths
    if not flags:return {'knots':None,'degree':0,'points':[(0,0,0,1)]}
    shape=c.shape() if flags&240 else None
    if shape:
        n,degree,knots=shape;c.align({0:4,1:1,2:2,5:4}[encoding])
    else:n,degree,knots=1,0,None
    points=[quaternion(c.take(widths[encoding]),encoding) for _ in range(n)]
    return {'knots':knots,'degree':degree,'points':points}


def decode(fields):
    n=fields[b'numberOfTransformTracks'];data=bytes(fields[b'data']);offsets=fields[b'blockOffsets'];floats=fields[b'floatBlockOffsets'];blocks=[]
    assert fields.get(b'numberOfFloatTracks',0)==0 and fields[b'maskAndQuantizationSize']==n*4
    for block,offset in enumerate(offsets):
        end=offsets[block+1] if block+1<len(offsets) else len(data)
        c=Cursor(data,offset,end);masks=[tuple(c.take(4)) for _ in range(n)];c.align(4);tracks=[]
        for quant,p,r,s in masks:
            pos=vector(c,p,quant&3,0)
            rot=rotation(c,r,quant>>2&15);c.align(4)
            scale=vector(c,s,quant>>6,1)
            tracks.append((pos,rot,scale))
        assert c.pos-offset==floats[block],(block,c.pos-offset,floats[block])
        tail=c.take(c.end-c.pos);assert not any(tail)
        blocks.append({'tracks':tracks,'end':c.pos,'masks':masks})
    return blocks


def basis(knots,degree,time):
    # Cox-de Boor basis recurrence over the entire knot vector, not the Rust
    # sampler's local de Boor control-polygon evaluation.
    n=len(knots)-degree-1;time=min(max(time,knots[degree]),knots[n])
    if time==knots[n]:return [0.]*(n-1)+[1.]
    weights=[float(knots[i]<=time<knots[i+1]) for i in range(len(knots)-1)]
    for order in range(1,degree+1):
        row=[]
        for i in range(len(weights)-1):
            a,b=knots[i+order]-knots[i],knots[i+order+1]-knots[i+1]
            row.append(((time-knots[i])/a*weights[i] if a else 0)+((knots[i+order+1]-time)/b*weights[i+1] if b else 0))
        weights=row
    assert len(weights)==n and abs(sum(weights)-1)<1e-8
    return weights


def sample_track(track,frame):
    p,r,s=track
    def vectors(v):
        w=basis(v['knots'],v['degree'],frame) if v['knots'] else None
        return tuple(a[0] if len(a)==1 else sum(x*y for x,y in zip(a,w)) for a in v['channels'])
    w=basis(r['knots'],r['degree'],frame) if r['knots'] else [1.]
    q=[sum(point[i]*weight for point,weight in zip(r['points'],w)) for i in range(4)]
    length=math.sqrt(sum(v*v for v in q));assert length>0
    return vectors(p)+tuple(v/length for v in q)+vectors(s)


if __name__=='__main__':
    from observe_tagfile import read
    import sys
    o=read(sys.argv[1])
    for i,obj in enumerate(o.objects):
        if obj and o.classes[obj['class']]['name']==b'hkaSplineCompressedAnimation':
            blocks=decode(obj['fields']);print('animation',i,'blocks',len(blocks),'tracks',len(blocks[0]['tracks']))
            for j,b in enumerate(blocks):print('block',j,'end',b['end'],'first-track',sample_track(b['tracks'][0],0))
