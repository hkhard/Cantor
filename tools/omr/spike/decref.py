# Reference greedy decode for staff 0 with the plain fp32 CPU models: the
# token ids (rhythm, pitch) the Rust probe must reproduce.
import numpy as np, onnxruntime as ort, json, glob, time
import os, homr
H=os.path.join(os.path.dirname(homr.__file__), 'transformer') + '/'
enc=ort.InferenceSession(glob.glob(H+'encoder_*.onnx')[0]); dec=ort.InferenceSession(glob.glob(H+'decoder_*.onnx')[0])
x=np.fromfile('olaf/enc_in_0.f32','<f4').reshape(1,1,256,1280)
t=time.perf_counter(); ctx=enc.run(None,{'input':x})[0]; te=time.perf_counter()-t
names=[o.name for o in dec.get_outputs()]
cache=[np.zeros((1,8,0,64),np.float32) for _ in range(32)]
last={'rhythms':1,'pitchs':0,'lifts':0,'articulations':0,'slurs':0}; ids=[]
t=time.perf_counter()
for step in range(608):
    feed={k:np.array([[v]],np.int64) for k,v in last.items()}
    feed['context']=ctx if step==0 else ctx[:,:1]; feed['cache_len']=np.array([step],np.int64)
    for i,c in enumerate(cache): feed[f'cache_in{i}']=c
    out=dec.run(None,feed); cache=out[7:]
    r=int(out[0][0,-1].argmax()); p=int(out[1][0,-1].argmax())
    if r==2: break
    ids.append([r,p]); last={'rhythms':r,'pitchs':p,'lifts':int(out[2][0,-1].argmax()),'articulations':int(out[4][0,-1].argmax()),'slurs':int(out[5][0,-1].argmax())}
td=time.perf_counter()-t
json.dump(ids,open('olaf/dec_ref.json','w')); np.save('olaf/ctx0.npy',ctx)
print('enc',round(te,3),'s dec',round(td,3),'s steps',len(ids), ids[:6])
