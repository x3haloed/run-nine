#!/usr/bin/env python3
"""Independent exact-rational phase / Decimal140 tangent oracle for captures."""
import json
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext,ROUND_HALF_EVEN
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def audit(native,runtime=None):
    with localcontext() as ctx:
        ctx.prec=140;log_two=Decimal(2).ln();period=Fraction(0xc90fdaa22168c235,1<<63)
        def oracle(h):
            v=int(h,16);se=v>>64;exponent=se&32767;mant=v&((1<<64)-1);negative=bool(se&32768)
            if exponent==32767:return None
            if not mant:return f'{se&32768:04x}0000000000000000'
            x=Fraction(mant)*Fraction(2)**(exponent-16383-63)
            q,rem=divmod(x,period)
            if rem*2>period or(rem*2==period and q&1):q+=1;rem-=period
            if not rem:return f'{32768 if negative else 0:04x}0000000000000000'if not q&1 else f'{(32768 if negative else 0)|32767:04x}0000000000000000'
            r=Decimal(abs(rem.numerator))/Decimal(rem.denominator)
            def series(cosine):
                term=Decimal(1) if cosine else r;y=term
                for k in range(1,256):
                    term=term*r*r/Decimal((2*k-1)*(2*k) if cosine else (2*k)*(2*k+1))
                    after=y-term if k&1 else y+term
                    if after==y:break
                    y=after
                else:raise AssertionError('Tangent oracle series failed')
                return y
            sine=series(False);cosine=series(True);y=cosine/sine if q&1 else sine/cosine
            negative=negative^bool(rem<0)^bool(q&1);sign=32768 if negative else 0
            e=int(y.ln()/log_two)
            while Decimal(2)**e>y:e-=1
            while Decimal(2)**(e+1)<=y:e+=1
            if e < -16383:m=int((y*Decimal(2)**16446).to_integral_value(rounding=ROUND_HALF_EVEN));encoded=0
            else:
                m=int((y*Decimal(2)**(63-e)).to_integral_value(rounding=ROUND_HALF_EVEN))
                if m==1<<64:m>>=1;e+=1
                encoded=e+16383
            return f'{sign|encoded:04x}{m:016x}'
        seen={};counts={'audited_rows':0,'unique_audited_inputs':0,'runtime_correctly_rounded_rows':0,'native_correctly_rounded_rows':0,'native_differing_rows':0};errors=[];native_examples=[]
        for section in ['tan','dense']:
            for i,n in enumerate(native[section]):
                h=n['before_hex']
                if h not in seen:seen[h]=oracle(h)
                expected=seen[h]
                if expected is None:continue
                counts['audited_rows']+=1
                if runtime is not None:
                    actual=runtime[section][i]['after_hex']
                    if actual==expected:counts['runtime_correctly_rounded_rows']+=1
                    else:errors.append({'section':section,'index':i,'input':h,'expected':expected,'actual':actual})
                if n['after_hex']==expected:counts['native_correctly_rounded_rows']+=1
                else:
                    counts['native_differing_rows']+=1
                    if len(native_examples)<8:native_examples.append({'input':h,'expected':expected,'native':n['after_hex']})
        counts['unique_audited_inputs']=sum(v is not None for v in seen.values())
        return {'oracle':'Exact Fraction reduction by packed extended pi/2, independent Decimal140 sine/cosine series and quotient, and nearest-even encoding','counts':counts,'runtime_errors':errors,'native_difference_examples':native_examples,'scope':'Captured finite inputs only with SANE packed-pi period. Not mathematical tangent with infinitely precise pi or a full-domain proof.'}
def main():
    p=OUT/'SANETanProbe-runtime-raw.json';r=audit(json.loads((OUT/'SANETanProbe-v1-reference.json').read_text()),json.loads(p.read_text())if p.exists()else None);(OUT/'SANETan-high-precision-audit.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));assert not r['runtime_errors']
if __name__=='__main__':main()
