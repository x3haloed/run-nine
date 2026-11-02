#include "Menus.r"
resource 'MENU' (21000) {
 21000,textMenuProc;allEnabled,enabled;"Alpha";
 {"New",noIcon,"N",noMark,plain;"Quit",noIcon,"Q",noMark,plain;}
};
resource 'MENU' (21001) {
 21001,textMenuProc;allEnabled,enabled;"Beta";
 {"Copy",noIcon,"C",noMark,plain;"Cut",noIcon,"X",noMark,plain;}
};
resource 'MBAR' (21000) {{21000,21001};};
resource 'MBAR' (21001) {{ };};
resource 'MBAR' (21002) {{21000,30000,21001};};
