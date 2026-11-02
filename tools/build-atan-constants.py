#!/usr/bin/env python3
"""Own Q112 pi constants from Machin's identity; no native numeric data."""
from decimal import Decimal,localcontext,ROUND_HALF_EVEN
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    with localcontext() as context:
        context.prec=140
        def atan(x):
            total=x;power=x;square=x*x
            for k in range(1,1000):
                power=-power*square;after=total+power/Decimal(2*k+1)
                if after==total:return total
                total=after
            raise RuntimeError('Constant series failed to converge')
        pi=16*atan(Decimal(1)/5)-4*atan(Decimal(1)/239)
        source='// Generated independently by tools/build-atan-constants.py.\n'
        for name,denom in [('PI_OVER_4',4),('PI_OVER_2',2)]:
            value=int((pi/denom*Decimal(2)**112).to_integral_value(rounding=ROUND_HALF_EVEN))
            source+=f'const {name}:u128=0x{value:x};\n'
    path=ROOT/'compat-core/src/atan_constants.rs'
    if '--check' in sys.argv:assert path.read_text()==source,'Changed arctangent constants'
    else:path.write_text(source)
    print('Q112 pi constants derived from Machin identity')
if __name__=='__main__':main()
