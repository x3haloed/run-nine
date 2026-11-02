#!/usr/bin/env python3
"""Independent Decimal140 audit of ordinary positive native power results.

This characterizes native accuracy; it does not assume correctly rounded power.
Special values, negative bases, zeros and under/overflow are outside this audit.
"""
from decimal import Decimal,localcontext
from pathlib import Path
import json
ROOT=Path(__file__).resolve().parents[1]
def main():
    rows=json.loads((ROOT/'outputs/SANEPowerProbe-v1-reference.json').read_text())['records']
    groups={k:{'pairs':0,'max_error_ulp':Decimal(0),'worst_kind':None}for k in ['all_positive_ordinary','bounded_exponent_abs_le_8','dense_curve_5_over_4']}
    with localcontext()as c:
        c.prec=140;c.Emax=9999999;c.Emin=-9999999
        def value(s):
            b=bytes.fromhex(s);e=int.from_bytes(b[:2],'big')
            return Decimal(-1 if e&0x8000 else 1)*Decimal(int.from_bytes(b[2:],'big'))*Decimal(2)**((e&0x7fff)-16383-63)
        for i in list(range(0,9900,45))+list(range(9900,len(rows),3)):
            r=rows[i];base=value(r['dest_before_hex']);y=value(r['source_before_hex'])
            b=bytes.fromhex(r['dest_after_hex']);e=int.from_bytes(b[:2],'big')&0x7fff
            if not(0<base and int(r['dest_before_hex'][:4],16)<0x7fff and 0<e<0x7fff and int.from_bytes(b[2:],'big')):continue
            expected=base**y;actual=value(r['dest_after_hex'])
            error=abs(actual-expected)/(Decimal(2)**(e-16383-63))
            keys=['all_positive_ordinary']
            if abs(y)<=8:keys.append('bounded_exponent_abs_le_8')
            if r['kind']>=220:keys.append('dense_curve_5_over_4')
            for key in keys:
                g=groups[key];g['pairs']+=1
                if error>g['max_error_ulp']:g['max_error_ulp']=error;g['worst_kind']=r['kind']
    for g in groups.values():g['max_error_ulp']=str(g['max_error_ulp'])
    result={'decimal_precision':140,'scope':'Native accuracy characterization only, not a runtime comparison or global error bound. Positive ordinary finite nonzero results only; selected unique owned pairs.','groups':groups}
    (ROOT/'outputs/SANEPower-independent-audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))
if __name__=='__main__':main()
