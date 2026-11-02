#include <QuickDraw.h>
#include <Menus.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
static char *geometry(char *p,MenuHandle m){return p+sprintf(p,"[%d,%d]",(**m).menuWidth,(**m).menuHeight);}
int main(void){MenuHandle m,s;Handle base;char *p=output;short file;long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();base=GetMenuBar();m=NewMenu(21000,"\pItems");AppendMenu(m,"\pFirst/A;Second;Third");InsertMenu(m,0);CalcMenuSize(m);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"menu-semantics-v1\",\"menus\":{\"geometry\":[");p=geometry(p,m);SetItemCmd(m,2,'X');p+=sprintf(p,",");p=geometry(p,m);SetItemMark(m,2,18);p+=sprintf(p,",");p=geometry(p,m);SetItemIcon(m,2,1);p+=sprintf(p,",");p=geometry(p,m);SetItemStyle(m,2,1);p+=sprintf(p,",");p=geometry(p,m);SetMenuItemText(m,2,"\pLonger label");p+=sprintf(p,",");p=geometry(p,m);p+=sprintf(p,"]");
 s=NewMenu(42,"\pSub");AppendMenu(s,"\pNested/N");InsertMenu(s,-1);p+=sprintf(p,",\"unlinked\":%lu",MenuKey('n'));SetItemCmd(m,3,27);SetItemMark(m,3,42);p+=sprintf(p,",\"linked\":%lu",MenuKey('n'));DisableItem(m,3);p+=sprintf(p,",\"parent_item_disabled\":%lu",MenuKey('n'));EnableItem(m,3);DisableItem(m,0);p+=sprintf(p,",\"parent_menu_disabled\":%lu",MenuKey('n'));EnableItem(m,0);DisableItem(s,0);p+=sprintf(p,",\"submenu_disabled\":%lu",MenuKey('n'));EnableItem(s,0);DisableItem(s,1);p+=sprintf(p,",\"submenu_item_disabled\":%lu",MenuKey('n'));EnableItem(s,1);SetItemMark(m,3,0);p+=sprintf(p,",\"unlinked_again\":%lu",MenuKey('n'));SetMenuBar(base);DisposeHandle(base);DisposeMenu(m);DisposeMenu(s);p+=sprintf(p,"}}\n");
 e=Create("\pMenuSemanticsProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pMenuSemanticsProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
