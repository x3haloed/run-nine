#include "Windows.r"
resource 'WIND' (22000) {{70,90,190,350},documentProc,invisible,goAway,0x12345678,"Default",noAutoCenter};
resource 'WIND' (22001) {{70,90,190,350},documentProc,invisible,goAway,0x12345678,"Blue",noAutoCenter};
resource 'WIND' (22002) {{70,90,190,350},plainDBox,invisible,noGoAway,0x12345678,"Black",noAutoCenter};
/* Owned part/color tables, independent of theme assets. */
data 'wctb' (22001) {$"00000000 0000 0004 0000 0000 0000 FFFF 0001 FFFF 0000 0000 0002 0000 FFFF 0000 0003 EEEE AAAA 1111 0004 1111 2222 3333"};
data 'wctb' (22002) {$"00000000 0000 0004 0000 0000 0000 0000 0001 FFFF 0000 0000 0002 0000 FFFF 0000 0003 EEEE AAAA 1111 0004 1111 2222 3333"};
data 'wctb' (22003) {$"00000000 0000 0004 0000 0000 FFFF 0000 0001 FFFF 0000 0000 0002 0000 FFFF 0000 0003 EEEE AAAA 1111 0004 1111 2222 3333"};
