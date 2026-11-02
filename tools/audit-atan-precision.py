#!/usr/bin/env python3
"""Independent 140-digit Decimal audit of retained packed-extended results."""
from decimal import Decimal,localcontext,ROUND_HALF_EVEN,ROUND_FLOOR
from pathlib import Path
import json
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def audit(native,runtime):
    with localcontext()as context:
        context.prec=140
        def series(x):
            total=x;power=x;square=x*x
            for k in range(1,2000):
                power=-power*square;value=total+power/Decimal(2*k+1)
                if value==total:return total
                total=value
            raise ValueError('Oracle did not converge')
        pi=16*series(Decimal(1)/5)-4*series(Decimal(1)/239);log2=Decimal(2).ln()
        def atan(x):
            if x>1:return pi/2-atan(1/x)
            if x>Decimal('.5'):return pi/4-series((1-x)/(1+x))
            return series(x)
        def oracle(text):
            b=bytes.fromhex(text);word=int.from_bytes(b[:2],'big');m=int.from_bytes(b[2:],'big');exp=word&0x7fff;sign=word&0x8000
            if exp==0x7fff:
                if m&0x7fffffffffffffff:return None
                y=pi/2
            elif m==0:return sign.to_bytes(2,'big').hex()+'0000000000000000'
            else:y=atan(Decimal(m)*Decimal(2)**(exp-16383-63))
            exponent=int((y.ln()/log2).to_integral_value(rounding=ROUND_FLOOR))
            while Decimal(2)**exponent>y:exponent-=1
            while Decimal(2)**(exponent+1)<=y:exponent+=1
            if exponent < -16383:mant=int((y*Decimal(2)**16446).to_integral_value(rounding=ROUND_HALF_EVEN));encoded_exp=0
            else:
                mant=int((y*Decimal(2)**(63-exponent)).to_integral_value(rounding=ROUND_HALF_EVEN))
                if mant==1<<64:mant>>=1;exponent+=1
                encoded_exp=exponent+16383
            return (sign|encoded_exp).to_bytes(2,'big').hex()+mant.to_bytes(8,'big').hex()
        seen={};counts={'audited_rows':0,'unique_audited_inputs':0,'runtime_correctly_rounded_rows':0,'native_correctly_rounded_rows':0,'native_differing_rows':0};errors=[]
        for section in ['atan','dense']:
            for i,(n,r)in enumerate(zip(native[section],runtime[section])):
                source=n['before_hex']
                if source not in seen:seen[source]=oracle(source)
                expected=seen[source]
                if expected is None:continue
                counts['audited_rows']+=1
                if r['after_hex']==expected:counts['runtime_correctly_rounded_rows']+=1
                else:errors.append({'section':section,'index':i,'input':source,'expected':expected,'actual':r['after_hex']})
                if n['after_hex']==expected:counts['native_correctly_rounded_rows']+=1
                else:counts['native_differing_rows']+=1
        counts['unique_audited_inputs']=sum(x is not None for x in seen.values())
        return {'oracle':'Python standard-library Decimal,140 decimal digits; independent precision/arithmetic from Q112 runtime','counts':counts,'runtime_errors':errors,'scope':'Correct rounding observed on captured inputs, not a proof over all possible extended values. NaNs excluded from numerical audit; native bit/state comparisons cover them.'}
def main():
    n=json.loads((OUT/'SANEProbe-v3-reference.json').read_text());r=json.loads((OUT/'SANEProbe-runtime-raw.json').read_text());result=audit(n,r);(OUT/'SANE-high-precision-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert not result['runtime_errors']
if __name__=='__main__':main()
