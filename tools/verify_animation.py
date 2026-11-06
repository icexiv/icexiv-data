"""Read-only PAP extraction and all-track independent spline sample oracle.

Args: installed game/sqpack new-output-directory. No target execution, no parser
dependencies. Rust local de Boor is compared to Python Cox-de Boor basis sums.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import struct
import sys
from find_vfx_samples import standard
from observe_tagfile import read
from observe_spline import decode, sample_track


def main():
    root,out=map(Path,sys.argv[1:3]);assert not out.resolve().is_relative_to(root.parent.resolve())
    manifest=json.loads((Path(sys.argv[3]) if len(sys.argv)>3 else Path(__file__).resolve().parent.parent/'tests/fixtures/pap-observations.json').read_text())
    version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    out.mkdir(parents=True,exist_ok=False);reports=[]
    for ordinal,item in enumerate(manifest['samples']):
        sample_out=out/str(ordinal);sample_out.mkdir();source_identity=item.get('path',f'{item.get("dat")}@{item.get("offset")}')
        path=sample_out/'sample.pap'
        if 'dat'in item:
            with (root/'ffxiv/040000.win32.index').open('rb')as f:
                f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert (item['index_ordinal']+1)*16<=size
                f.seek(off+item['index_ordinal']*16);key,packed,_=struct.unpack('<QII',f.read(16));assert not packed&1 and f'{key:016x}'==item['index_hash']
                assert item['dat']==f'ffxiv/040000.win32.dat{(packed>>1)&7}'and item['offset']==(packed>>4)*128
            with (root/item['dat']).open('rb')as f:independent=standard(f,item['offset'])
            log=subprocess.check_output(['target/debug/examples/extract_entry',str(root),item['dat'],str(item['offset']),str(path)],text=True)
            assert path.read_bytes()==independent
        else:log=subprocess.check_output(['target/debug/examples/local_sqpack',str(root),item['path'],str(path)],text=True)
        (sample_out/'extraction.txt').write_text(log)
        raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==item['sha256'] and len(raw)==item['bytes']
        if item.get('empty'):
            count,=struct.unpack_from('<H',raw,8);records,start,end=struct.unpack_from('<III',raw,14)
            assert raw[:8]==b'pap \x01\x00\x02\x00'and count==0 and records==start==item['tagfile_offset']and end==item['tagfile_end']
            assert end-start==item['padding_bytes']and end%16==0 and not any(raw[start:end])
            actual=subprocess.check_output(['target/debug/examples/validate_animation',str(path)],text=True)
            assert actual==f'empty\t0\t{end-start}\n'and hashlib.sha256(actual.encode()).hexdigest()==item['rust_rows_sha256']
            (sample_out/'rust.tsv').write_text(actual);reports.append({'path_or_entry':source_identity,'sha256':item['sha256'],'empty':True,'padding_bytes':end-start,'rust_rows_sha256':item['rust_rows_sha256']});continue
        observer=read(path,max_array_length=item.get('max_array_length',1000000),max_values=item.get('observer_max_values',2000000));assert len(observer.classes)-1==item['classes'] and len(observer.objects)-1==item['objects']
        start,end=struct.unpack_from('<II',raw,18);assert start==item['tagfile_offset'] and end==item['tagfile_end']
        padding=end-start-observer.pos
        if 'padding_bytes'in item:assert padding==item['padding_bytes']
        container=next(o for o in observer.objects if o and observer.classes[o['class']]['name']==b'hkaAnimationContainer')
        assert [v['ref'] for v in container['fields'][b'animations']]==item['animation_object_ids']
        assert [v['ref'] for v in container['fields'][b'bindings']]==item['binding_object_ids']
        blocks={identity:decode(observer.objects[identity]['fields']) for identity in item['animation_object_ids']}
        command=['target/debug/examples/validate_animation',str(path)]
        if 'expected_default_error'in item:
            default=subprocess.run(command,capture_output=True,text=True);assert default.returncode!=0 and item['expected_default_error']in default.stderr
            (sample_out/'default-budget-error.txt').write_text(default.stderr)
        if 'max_array_length'in item:command.append(str(item['max_array_length']))
        if item.get('sample_boundaries'):command.append('--block-boundaries')
        actual=subprocess.check_output(command,text=True)
        (sample_out/'rust.tsv').write_text(actual)
        expected=[];count=0;error=0;seen=set();animations=[];time_seen=set();bindings_seen=[]
        for line in actual.splitlines():
            row=line.split('\t')
            if row[0]=='animation':
                _,index,binding,identity,frames,block_count=row
                index,binding,identity,frames,block_count=map(int,[index,binding,identity,frames,block_count])
                assert identity==item['animation_object_ids'][index] and binding==item['binding_object_ids'][index]
                fields=observer.objects[binding]['fields'];assert fields[b'animation']=={'ref':identity}
                assert frames==observer.objects[identity]['fields'][b'numFrames'] and block_count==len(blocks[identity])
                animations.append(identity);continue
            if row[0]=='binding':
                _,identity,name,hint,indices,skeleton=row;identity=int(identity)
                binding=item['binding_object_ids'][item['animation_object_ids'].index(identity)]
                fields=observer.objects[binding]['fields']
                assert list(map(int,indices.split(',')))==fields[b'transformTrackToBoneIndices']
                assert int(hint)==fields.get(b'blendHint',0) and skeleton==fields[b'originalSkeletonName'].hex()
                index=item['animation_object_ids'].index(identity);record=raw[26+index*40:66+index*40]
                assert name==record[:32].split(b'\0',1)[0].hex();bindings_seen.append(identity);continue
            if row[0]=='time':
                _,identity,time,track,values=row;identity,track=int(identity),int(track);time=float(time)
                fields=observer.objects[identity]['fields'];seconds=max(0,min(time/1000,fields[b'duration']))
                frame=seconds/fields[b'frameDuration'];stride=fields[b'maxFramesPerBlock']-1
                block=min(int(frame//stride),len(blocks[identity])-1)
                wanted=sample_track(blocks[identity][block]['tracks'][track],frame-block*stride)
                got=list(map(float,values.split(',')));assert len(got)==10
                delta=max(abs(a-b) for a,b in zip(wanted,got));assert delta<2e-6
                error=max(error,delta);count+=10;key=(identity,time,track);assert key not in time_seen;time_seen.add(key)
                expected.append('time\t'+'\t'.join(map(str,key))+'\t'+','.join(format(v,'.17g') for v in wanted));continue
            _,identity,block,frame,track,values=row
            identity,block,track=map(int,[identity,block,track]);frame=float(frame)
            key=(identity,block,frame,track);assert key not in seen;seen.add(key)
            expected_values=sample_track(blocks[identity][block]['tracks'][track],frame)
            actual_values=list(map(float,values.split(',')));assert len(actual_values)==10
            delta=max(abs(a-b) for a,b in zip(expected_values,actual_values));assert delta<2e-6,(key,delta)
            error=max(error,delta);count+=10
            expected.append('\t'.join(map(str,key))+'\t'+','.join(format(v,'.17g') for v in expected_values))
        assert animations==item['animation_object_ids']
        expected_times=set()
        for identity,bs in blocks.items():
            fields=observer.objects[identity]['fields'];times={-1.,0.,125.,500.,566.6666,566.6667,700.,1000.,2500.}
            if item.get('sample_boundaries'):
                for block in range(1,len(bs)):
                    time=(block*(fields[b'maxFramesPerBlock']-1))*fields[b'frameDuration']*1000.;times.update([time-.0001,time,time+.0001])
                end=fields[b'duration']*1000.;times.update([end-.0001,end,end+1.])
            expected_times.update((identity,time,track)for time in times for track in range(len(bs[0]['tracks'])))
        assert bindings_seen==animations and time_seen==expected_times
        assert len(seen)==10*sum(len(b['tracks']) for a in blocks.values() for b in a)
        assert count==item['sampled_coefficients']
        assert hashlib.sha256(actual.encode()).hexdigest()==item['rust_rows_sha256']
        overlap=[]
        for identity,bs in blocks.items():
            stride=observer.objects[identity]['fields'][b'maxFramesPerBlock']-1
            for i in range(len(bs)-1):
                delta=max(abs(a-b) for t,u in zip(bs[i]['tracks'],bs[i+1]['tracks']) for a,b in zip(sample_track(t,stride),sample_track(u,0)))
                overlap.append({'object_id':identity,'blocks':[i,i+1],'max_component_difference':delta})
        maximum_overlap=max((v['max_component_difference']for v in overlap),default=0)
        if 'overlap_observations'in manifest:assert abs(maximum_overlap-manifest['overlap_observations'][ordinal])<1e-12
        else:assert maximum_overlap<0.002
        (sample_out/'python.tsv').write_text('\n'.join(expected)+'\n')
        reports.append({'path_or_entry':source_identity,'sha256':item['sha256'],'coefficients':count,'max_absolute_error':error,'padding_bytes':padding,'overlapping_frame_observations':overlap,
                         'rust_rows_sha256':hashlib.sha256(actual.encode()).hexdigest(),'millisecond_track_samples':len(time_seen),'sampling':'ten local coordinates per block plus nine base millisecond coordinates, every track and TRS component; complete binding maps'+('; every interior block boundary and both sides, duration and clamp endpoint'if item.get('sample_boundaries')else'')})
    report={'version':version,'platform':'Win32','python':sys.version,'samples':reports};text=json.dumps(report,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()
