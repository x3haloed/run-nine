#include <QuickDraw.h>
#include <Menus.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char output[16384];
static char *state(char *p,MenuHandle m,short item){short cmd=0x1234,mark=0x1234,icon=0x1234,style=0x1234;unsigned char text[256];short i;
 memset(text,0xa5,sizeof(text));GetItemCmd(m,item,&cmd);GetItemMark(m,item,&mark);GetItemIcon(m,item,&icon);GetItemStyle(m,item,&style);GetMenuItemText(m,item,text);
 p+=sprintf(p,"{\"count\":%d,\"values\":[%d,%d,%d,%d],\"text\":[",CountMItems(m),cmd,mark,icon,style);
 for(i=0;i<text[0];i++)p+=sprintf(p,"%s%u",i?",":"",text[i+1]);return p+sprintf(p,"]}");}
int main(void){MenuHandle m,sub;Handle base,bar;short values[]={0,127,128,255,256,-1},i,file;char *p=output;long count;OSErr err;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();base=GetMenuBar();m=NewMenu(21000,"\pItems");AppendMenu(m,"\pFirst/A;Second;Third");InsertMenu(m,0);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"menu-items-v1\",\"menus\":{\"initial\":");p=state(p,m,1);
 p+=sprintf(p,",\"set\":[");for(i=0;i<6;i++){SetItemCmd(m,2,values[i]);SetItemMark(m,2,values[i]);SetItemIcon(m,2,values[i]);SetItemStyle(m,2,values[i]);p+=sprintf(p,"%s",i?",":"");p=state(p,m,2);}p+=sprintf(p,"]");
 CheckItem(m,2,1);p+=sprintf(p,",\"checked\":");p=state(p,m,2);CheckItem(m,2,0);p+=sprintf(p,",\"unchecked\":");p=state(p,m,2);
 SetMenuItemText(m,2,"\pLonger label");p+=sprintf(p,",\"renamed\":");p=state(p,m,2);
 p+=sprintf(p,",\"invalid_zero\":");p=state(p,m,0);p+=sprintf(p,",\"invalid_high\":");p=state(p,m,99);
 sub=NewMenu(42,"\pSub");AppendMenu(sub,"\pNested/N");SetItemCmd(m,3,27);SetItemMark(m,3,42);InsertMenu(sub,-1);bar=GetMenuBar();
 p+=sprintf(p,",\"hierarchy_inserted\":[%d,%d,%lu]",GetMenuHandle(21000)==m,GetMenuHandle(42)==sub,MenuKey('n'));
 DeleteMenu(42);p+=sprintf(p,",\"hierarchy_deleted\":[%d,%lu]",GetMenuHandle(42)==0,MenuKey('n'));
 SetMenuBar(bar);DisposeHandle(bar);p+=sprintf(p,",\"hierarchy_restored\":[%d,%lu]",GetMenuHandle(42)==sub,MenuKey('n'));
 SetMenuBar(base);DisposeHandle(base);DisposeMenu(m);DisposeMenu(sub);p+=sprintf(p,"}}\n");
 err=Create("\pMenuItemsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pMenuItemsProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;}
