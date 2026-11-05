import numpy as np, librosa, soundfile as sf, scipy.signal as ss
sr=44100
b,_=librosa.load("sep/htdemucs/sunny/bass.wav",sr=sr,mono=True)
m,_=librosa.load("sunny.wav",sr=sr,mono=True)
rest=sum(librosa.load("sep/htdemucs/sunny/%s.wav"%s,sr=sr,mono=True)[0] for s in ("drums","other","vocals"))
def band(x,lo,hi):
    s,a=ss.butter(4,[lo/(sr/2),hi/(sr/2)],btype="band"); return ss.filtfilt(s,a,x)
e=lambda x: 10*np.log10(np.mean(x**2)+1e-12)
print("bass stem level %.1f dBFS; mix %.1f"%(e(b),e(m)))
print("bass stem energy above 400 Hz relative to its 40-300 Hz band: %.1f dB"%(e(band(b,400,4000))-e(band(b,40,300))))
print("40-300 Hz energy left in the rest (bleed the other way): %.1f dB vs the bass stem's"%(e(band(rest,40,300))-e(band(b,40,300))))
import torch, torchcrepe
y16=librosa.resample(b,orig_sr=sr,target_sr=16000)
f0,per=torchcrepe.predict(torch.tensor(y16)[None],16000,160,fmin=35,fmax=400,model="full",return_periodicity=True,batch_size=2048,device="cuda")
per=per[0].cpu().numpy(); r=librosa.feature.rms(y=y16,frame_length=640,hop_length=160)[0][:len(per)]
on=r>np.percentile(r,40)
print("pitch clarity (CREPE periodicity) on sounding frames: median %.2f, share > 0.6: %.0f%%"%(np.median(per[on]),100*np.mean(per[on]>0.6)))
# drum bleed: transient share in bass stem
env=librosa.onset.onset_strength(y=band(b,2000,8000),sr=sr)
envd=librosa.onset.onset_strength(y=band(librosa.load("sep/htdemucs/sunny/drums.wav",sr=sr,mono=True)[0],2000,8000),sr=sr)
n=min(len(env),len(envd)); print("cymbal/snare transients in the bass stem's 2-8 kHz vs the drums stem: correlation %.2f, level %.1f dB"%(np.corrcoef(env[:n],envd[:n])[0,1], e(band(b,2000,8000))-e(band(librosa.load('sep/htdemucs/sunny/drums.wav',sr=sr,mono=True)[0],2000,8000))))
