#!/usr/bin/env python3
"""Independent exact-rational phase and Decimal140 SANE-period cosine audit."""
import json
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext,ROUND_HALF_EVEN
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def audit(native,runtime):
    with localcontext() as ctx:
        ctx.prec=140;log_two=Decimal(2).ln();period=Fraction(0xc90fdaa22168c235,1<<63)
        def oracle(h):
            v=int(h,16);se=v>>64;exponent=se&32767;mant=v&((1<<64)-1);negative=bool(se&32768)
            if exponent==32767:return None
            if not mant:return '3fff8000000000000000'
            x=Fraction(mant)*Fraction(2)**(exponent-16383-63)
            q,rem=divmod(x,period)
            if rem*2>period or(rem*2==period and q&1):q+=1;rem-=period
            cosine=not bool(q&1);reduced=Decimal(abs(rem.numerator))/Decimal(rem.denominator)
            term=Decimal(1) if cosine else reduced;y=term
            for k in range(1,256):
                term=term*reduced*reduced/Decimal((2*k-1)*(2*k) if cosine else (2*k)*(2*k+1))
                after=y-term if k&1 else y+term
                if after==y:break
                y=after
            else:raise AssertionError('Cosine oracle series failed')
            negative=bool((q+1)&2)^bool(rem<0 and not cosine)^bool(negative and not y);sign=32768 if negative else 0
            if not y:return f'{sign:04x}0000000000000000'
            e=int(y.ln()/log_two)
            while Decimal(2)**e>y:e-=1
            while Decimal(2)**(e+1)<=y:e+=1
            if e < -16383:m=int((y*Decimal(2)**16446).to_integral_value(rounding=ROUND_HALF_EVEN));encoded=0
            else:
                m=int((y*Decimal(2)**(63-e)).to_integral_value(rounding=ROUND_HALF_EVEN))
                if m==1<<64:m>>=1;e+=1
                encoded=e+16383
            return f'{sign|encoded:04x}{m:016x}'
        seen={};counts={'audited_rows':0,'unique_audited_inputs':0,'runtime_correctly_rounded_rows':0,'native_correctly_rounded_rows':0,'native_differing_rows':0};errors=[]
        for section in ['cos','dense']:
            for i,(n,r) in enumerate(zip(native[section],runtime[section])):
                h=n['before_hex']
                if h not in seen:seen[h]=oracle(h)
                expected=seen[h]
                if expected is None:continue
                counts['audited_rows']+=1
                if r['after_hex']==expected:counts['runtime_correctly_rounded_rows']+=1
                else:errors.append({'section':section,'index':i,'input':h,'expected':expected,'actual':r['after_hex']})
                if n['after_hex']==expected:counts['native_correctly_rounded_rows']+=1
                else:counts['native_differing_rows']+=1
        counts['unique_audited_inputs']=sum(v is not None for v in seen.values())
        return {'oracle':'Exact Python Fraction reduction by packed extended pi/2, followed by standard-library Decimal at140digits and independent nearest-even encoding','counts':counts,'runtime_errors':errors,'scope':'Captured finite inputs only, with the documented SANE extended-pi period. This is not mathematical cosine with infinitely precise pi, a full-domain proof, or bit-identical native polynomial emulation.'}
def main():
    result=audit(json.loads((OUT/'SANECosProbe-v1-reference.json').read_text()),json.loads((OUT/'SANECosProbe-runtime-raw.json').read_text()));(OUT/'SANECos-high-precision-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert not result['runtime_errors']
if __name__=='__main__':main()
