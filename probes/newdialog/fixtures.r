#include "Dialogs.r"
resource 'DLOG' (128) {{70,90,250,450},dBoxProc,visible,goAway,0x12345678,128,"Owned dialog",noAutoCenter};
resource 'DLOG' (129) {{80,100,260,460},plainDBox,invisible,noGoAway,0x11223344,128,"Hidden dialog",noAutoCenter};
resource 'DLOG' (130) {{90,110,270,470},dBoxProc,visible,noGoAway,0x22334455,32760,"Missing items",noAutoCenter};
resource 'DITL' (128) {{
 {140,270,162,340},Button{enabled,"OK"};
 {20,20,40,160},CheckBox{enabled,"Checked"};
 {50,20,70,160},RadioButton{enabled,"Radio"};
 {80,20,100,240},StaticText{disabled,"Static text"};
 {110,20,130,240},EditText{enabled,"Edit text"};
}};
