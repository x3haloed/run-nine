#include "Dialogs.r"
resource 'ALRT' (128) {{50,70,170,430},128,{OK,visible,sound1,OK,visible,sound1,OK,visible,sound1,OK,visible,sound1},noAutoCenter};
resource 'ALRT' (129) {{50,70,170,430},128,{OK,visible,sound1,OK,visible,sound1,OK,visible,sound1,OK,visible,sound1},centerMainScreen};
resource 'ALRT' (130) {{50,70,170,430},128,{OK,visible,sound1,OK,visible,sound1,OK,visible,sound1,OK,visible,sound1},alertPositionMainScreen};
resource 'ALRT' (131) {{52,36,174,465},128,{OK,visible,sound1,OK,visible,sound1,OK,visible,sound1,OK,visible,sound1},alertPositionMainScreen};
resource 'DITL' (128) {{
 {85,270,105,340},Button{enabled,"OK"};
 {85,180,105,250},Button{enabled,"Cancel"};
 {15,65,70,340},StaticText{disabled,"Value ^0 and ^1."};
}};
