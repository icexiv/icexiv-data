"""Independent typed Data/texture/cumulative-emitter/reference observations.

Args: game/sqpack new-output-dir. Uses only struct and independent type-2
reconstruction. Known schema names classify fields; unknowns retain raw bytes.
"""
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from find_vfx_samples import standard
from verify_avfx import fields,label
from verify_weapon_binder import observe_binders

KINDS={(b'Emit',3):'EmitterSphere',(b'Emit',4):'EmitterCylinder',(b'Emit',5):'EmitterModel',(b'Ptcl',1):'ParticlePowder',(b'Ptcl',2):'ParticleWindmill',(b'Ptcl',5):'ParticleModel',(b'Ptcl',12):'ParticleDisc',(b'Ptcl',13):'ParticleLightModel',(b'Efct',0):'EffectorPointLight',(b'Bind',0):'BinderPoint'}
SHAPE_CURVES='IjS IjSR AnX AnY AnZ AnXR AnYR AnZR'
SCHEMAS={
 'EmitterSphere':{'int':'ROT GeMT DivX DivY','curve':'Rads '+SHAPE_CURVES},
 'EmitterCylinder':{'int':'ROT GeMT DivX DivY','curve':'Len Rad '+SHAPE_CURVES},
 'EmitterModel':{'int':'ROT GeMT MdNo','curve':SHAPE_CURVES},
 'ParticlePowder':{'small':'bMV bLoc bLgt','int':'LgtT','float':'CnOf'},
 'ParticleWindmill':{'int':'WUvT'},
 'ParticleModel':{'int':'MNRv MNRt MNRi FrsT DLT PLT bLgt bShp','bytes':'MdNo','curve':'NoAn Moph FrC FrCR','group':'FrRt ColB ColE'},
 'ParticleLightModel':{'small':'MNO'},
 'ParticleDisc':{'int':'PrtC PCnU PCnV SS','float':'PIFU','curve':'Ang AngR HBI HBIR HEI HEIR HBO HBOR HEO HEOR WB WBR WE WER RB RBR RE RER','group':'CEI CEO'},
 'EffectorPointLight':{'int':'Attn bSdw bChS bMpS bMvS','float':'SCDN SCDF','curve':'DstS','group':'Col Rot Pos'},
 'BinderPoint':{'curve':'SpS SpSR'}}

KINDS.update({(b'Emit',1):'EmitterCone',(b'Emit',2):'EmitterConeModel',
 (b'Ptcl',3):'ParticleLine',(b'Ptcl',4):'ParticleLaser',(b'Ptcl',6):'ParticlePolyline',(b'Ptcl',8):'ParticleQuad',(b'Ptcl',10):'ParticleDecal',(b'Ptcl',11):'ParticleDecalRing',(b'Ptcl',14):'ParticleModelSkin',(b'Ptcl',15):'ParticleDissolve',
 (b'Efct',1):'EffectorDirectionalLight',(b'Efct',3):'EffectorChromaticAberration',(b'Efct',4):'EffectorGaussianBlur',(b'Efct',5):'EffectorDirectionalBlur',(b'Efct',6):'EffectorCameraQuakeVariable',(b'Efct',7):'EffectorRadialBlur',(b'Efct',9):'EffectorCameraQuake',
 (b'Bind',1):'BinderLinear',(b'Bind',2):'BinderSpline',(b'Bind',3):'BinderCamera',(b'Bind',4):'BinderLinearAdjust'})
SCHEMAS.update({
 'EmitterCone':{'int':'ROT','curve':SHAPE_CURVES+' InS InSR OuS OuSR IjA IjAR'},
 'EmitterConeModel':{'int':'ROT GeMT DivX DivY','curve':SHAPE_CURVES+' Rad RadR IjA IjAR'},
 'ParticleLine':{'int':'LnCT','curve':'Len LenR','group':'ColB ColE'},
 'ParticleLaser':{'curve':'Len LenR Wdt WdtR'},
 'ParticlePolyline':{'int':'LnCT NBBA BWpT PnC PnCC PnED bEdg bNtB BdWp bCtg bCtr TagN bSpl bLcl','curve':'CF CFR Wd WdR WdB WdBR WdC WdCR WdE WdER Len LenR Sft SftR PnDs','group':'ColB ColC ColE'},
 'ParticleQuad':{'int':'SS','small':'bMP'},
 'ParticleDecal':{'int':'DDTT','float':'SS'},
 'ParticleDecalRing':{'int':'DDTT','float':'SS RF','curve':'WID WIDR'},
 'ParticleModelSkin':{'int':'FrsT AuTT bCM','curve':'FrC FrCR SEM SEMR EEM EEMR','group':'FrRt ColB ColE UVPD'},
 'ParticleDissolve':{'int':'bRev BST NPT DTT','curve':'EroR EdW EdCW ECMP Int','group':'EdC'},
 'EffectorDirectionalLight':{'curve':'Pow PowR','group':'Amb Col Rot'},
 'EffectorCameraQuake':{'curve':'Att AttR RdO RdOR RdI RdIR','group':'Rot Pos'},
 'EffectorCameraQuakeVariable':{'curve':'Att AttR RdO RdOR RdI RdIR','group':'Rot Pos'},
 'EffectorChromaticAberration':{'int':'FaBP','float':'FSDc FEDc','curve':'CSR CSS CGE Str Gra IRad ORad'},
 'EffectorGaussianBlur':{'int':'FaBP','float':'FSDc FEDc','curve':'Len Str Gra IRad ORad'},
 'EffectorDirectionalBlur':{'int':'FaBP bOS','float':'FSDc FEDc','curve':'Len Str AStr Ang'},
 'EffectorRadialBlur':{'int':'FaBP','float':'FSDc FEDc','curve':'Len Str Gra IRad ORad'},
 'BinderLinear':{'curve':'COF COFR'},'BinderSpline':{'curve':'COF COFR'},'BinderCamera':{'curve':'Dst DstR'},'BinderLinearAdjust':{'curve':'COF'}})


def one(fs,tag):
    matched=[r for r in fs if label(r[0])==tag];assert len(matched)<=1
    return matched[0]if matched else None
def number(fs,tag):
    r=one(fs,tag)
    return struct.unpack('<i',r[1])[0]if r else None
def optional(v):return 'none'if v is None else str(v)
def nested(r):return fields(r[1],r[2]+8)
def emitter_records(fs):
    e,p=number(fs,b'EmCn'),number(fs,b'PrCn');assert e>=0 and p>=0
    candidates=[r for r in fs if label(r[0])in[b'ItPr',b'ItEm']]
    assert sum(label(r[0])==b'ItEm'for r in candidates)==e and len(candidates)==e+p
    records=[]
    for block in candidates:
        columns=nested(block);starts=[i for i,r in enumerate(columns)if label(r[0])==b'bEnb'];assert starts and starts[0]==0
        records.append(('Emitter'if label(block[0])==b'ItEm'else'Particle',columns[starts[-1]:]))
    return records
def textures(fs):return [r for r in fs if label(r[0])in [b'TC1',b'TC2',b'TC3',b'TC4',b'TN',b'TR',b'TD',b'TP']]


def observe(raw):
    root=fields(raw[8:],8);nodes=[r for r in root if label(r[0])in[b'Schd',b'TmLn',b'Emit',b'Ptcl',b'Efct',b'Bind']]
    counts=Counter(label(t)for t,_,_,_ in root);target={n:counts[t]for n,t in [('Timeline',b'TmLn'),('Emitter',b'Emit'),('Particle',b'Ptcl'),('Effector',b'Efct'),('Binder',b'Bind'),('Model',b'Modl'),('Texture',b'Tex')]}
    lines=[];data_nodes=[];data_kinds=Counter();typed_parameters=0;unknown=0
    for node in nodes:
        t,_,owner,_=node;name=label(t);fs=nested(node);d=one(fs,b'Data');typed=[]
        if d is None:lines.append(f'data\t{owner}\tabsent');data_nodes.append((None,[]))
        else:
            variant=number(fs,{b'Emit':b'EVT',b'Ptcl':b'PrVT',b'Efct':b'EfVT',b'Bind':b'BnVr'}.get(name,b'UNKN'))
            kind=KINDS.get((name,variant))
            if kind is None:lines.append(f'data\t{owner}\tunknown\t{optional(variant)}');unknown+=1;data_nodes.append((None,[]))
            else:
                children=nested(d);lines.append(f'data\t{owner}\t{kind}\t{len(children)}');data_kinds[kind]+=1
                for child in children:
                    tag,p,off,_=child;schema=SCHEMAS[kind];encoding=next((enc for enc,names in schema.items()if label(tag).decode()in names.split()),'unknown');value=''
                    if encoding=='small':encoding='byte'if len(p)==1 else 'int'
                    if encoding=='int':value=str(struct.unpack('<i',p)[0])
                    elif encoding=='byte':assert len(p)==1;value=str(p[0])
                    elif encoding=='float':assert len(p)==4;value=f'{struct.unpack("<I",p)[0]:08x}'
                    elif encoding=='curve':
                        curve=nested(child);keys=one(curve,b'Keys')[1];assert len(keys)%16==0 and number(curve,b'KeyC')==len(keys)//16;value=str(len(keys)//16)
                    elif encoding=='group':value=str(len(nested(child)))
                    else:value=p.hex()
                    lines.append(f'parameter\t{owner}\t{off}\t{tag.hex()}\t{encoding}\t{value}');typed_parameters+=encoding!='unknown';typed.append((child,encoding))
                data_nodes.append((kind,typed))
        if name==b'Emit':
            for i,(kind,row)in enumerate(emitter_records(fs)):
                values=[number(row,tag)for tag in[b'bEnb',b'TgtB',b'CrTm',b'CrCn',b'CrPr']]
                lines.append(f'emitter_item\t{owner}\t{i}\t{kind}\t'+'\t'.join(map(optional,values)))
        if name==b'Ptcl':
            for slot in textures(fs):
                f=nested(slot);mask=one(f,b'TLst');curve=one(f,b'TxN');nkeys=len(one(nested(curve),b'Keys')[1])//16 if curve else None
                lines.append(f'texture_slot\t{owner}\t{slot[2]}\t{slot[0].hex()}\t'+ '\t'.join(optional(number(f,tag))for tag in [b'bEna',b'TxNo',b'UvSN'])+'\t'+(mask[1].hex()if mask else'none')+'\t'+optional(nkeys))
    refs=[];animated=0
    def add(owner,field,kind,index,element=0):
        status='Negative'if index<0 else('InRange'if index<target[kind]else'OutOfRange')
        refs.append((owner,field,element,kind,index,status))
    for node,(data_kind,params)in zip(nodes,data_nodes):
        t,_,owner,_=node;name=label(t);fs=nested(node)
        if name in[b'Schd',b'TmLn']:
            for tag in[b'Item',b'Trgr']:
                candidates=[r for r in fs if label(r[0])==tag]
                if candidates:
                    for f in nested(candidates[-1]):
                        kind={b'TlNo':'Timeline',b'EmNo':'Emitter',b'EfNo':'Effector',b'BdNo':'Binder'}.get(label(f[0]))
                        if kind:add(owner,f[2],kind,struct.unpack('<i',f[1])[0])
        if name==b'Emit':
            f=one(fs,b'EfNo')
            if f:add(owner,f[2],'Effector',struct.unpack('<i',f[1])[0])
            for kind,row in emitter_records(fs):
                f=one(row,b'TgtB')
                if f:add(owner,f[2],kind,struct.unpack('<i',f[1])[0])
        if name==b'Ptcl':
            for slot in textures(fs):
                f=nested(slot);animated+=one(f,b'TxN')is not None;index=one(f,b'TxNo');mask=one(f,b'TLst')
                if index:add(owner,index[2],'Texture',struct.unpack('<i',index[1])[0])
                if mask:
                    for i,v in enumerate(mask[1]):add(owner,mask[2],'Texture',v,i)
        for f,encoding in params:
            if (data_kind in['EmitterModel','ParticleModel']and label(f[0])==b'MdNo')or(data_kind=='ParticleLightModel'and label(f[0])==b'MNO'):
                values=list(f[1])if encoding in['byte','bytes']else[struct.unpack('<i',f[1])[0]]
                for i,v in enumerate(values):add(owner,f[2],'Model',v,i)
    lines.append(f'report\t{len(refs)}\t{animated}\t{unknown}')
    for r in refs:lines.append('reference\t'+'\t'.join(map(str,r)))
    return '\n'.join(lines)+'\n',{'typed_data_kinds':dict(data_kinds),'typed_parameters':typed_parameters,'references':len(refs),'reference_statuses':dict(Counter(r[-1]for r in refs)),'animated_texture_curves':animated,'unknown_data_nodes':unknown,'out_of_range_observations':[dict(zip(['owner','field','element','target','index','status'],r))for r in refs if r[-1]=='OutOfRange']}


def main():
    root,out=map(Path,sys.argv[1:3]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest_path=Path(sys.argv[3])if len(sys.argv)>3 else Path(__file__).resolve().parent.parent/'tests/fixtures/avfx-observations.json'
    manifest=json.loads(manifest_path.read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];reports=[]
    for i,s in enumerate(manifest['samples']):
        path=out/f'{i}.avfx';subprocess.check_output(['target/debug/examples/extract_entry',str(root),s['dat'],str(s['offset']),str(path)],text=True)
        with (root/s['dat']).open('rb')as f:raw=standard(f,s['offset'])
        assert raw==path.read_bytes()and hashlib.sha256(raw).hexdigest()==s['sha256']
        if 'referenced_cube'in manifest and i==manifest['referenced_cube']['owner_sample']:
            assert manifest['referenced_cube']['path'].encode()in[p.split(b'\0',1)[0]for t,p,_,_ in fields(raw[8:],8)if label(t)==b'Tex']
        expected,report=observe(raw);actual=subprocess.check_output(['target/debug/examples/validate_avfx_assets',str(path)],text=True);(out/f'{i}.rust.tsv').write_text(actual);(out/f'{i}.python.tsv').write_text(expected);assert actual==expected,f'asset observation mismatch for {i}'
        report.update(s);report['asset_rows_sha256']=hashlib.sha256(actual.encode()).hexdigest()
        if'asset_rows_sha256'in manifest:assert report['asset_rows_sha256']==manifest['asset_rows_sha256'][i]
        expected_binders,binder_stats=observe_binders(raw);actual_binders=subprocess.check_output(['target/debug/examples/validate_binder',str(path)],text=True);assert actual_binders==expected_binders
        (out/f'{i}.binder.rust.tsv').write_text(actual_binders);(out/f'{i}.binder.python.tsv').write_text(expected_binders);report['binder_properties']=binder_stats;report['binder_rows_sha256']=hashlib.sha256(expected_binders.encode()).hexdigest()
        if'binder_rows_sha256'in manifest:assert report['binder_rows_sha256']==manifest['binder_rows_sha256'][i]
        reports.append(report)
    text=json.dumps({'version':version,'platform':'Win32','samples':reports,'comparison':'all typed Data, emitter-item, texture-slot and reference rows equal'},indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()
