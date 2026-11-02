#include <QuickDraw.h>
#include <Fonts.h>
#include <Menus.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char output[262144];
static char *text(char *p,unsigned char *s){short j;p+=sprintf(p,"[");for(j=0;j<s[0];j++)p+=sprintf(p,"%s%u",j?",":"",s[j+1]);return p+sprintf(p,"]");}
int main(void){short ids[]={0,1,3,4,2002,32767,-1},i,j,number,file,items;unsigned char name[256];unsigned char *queries[]={"\pChicago","\pchicago","\pGeneva","\pGENEVA","\pMonaco","\pCharcoal","\pSystem","\pApplication","\pnot-a-font"};MenuHandle menu;char *p=output;long count;OSErr err;OSType types[]={'FONT','FOND'};
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();p+=sprintf(p,"{\"schema\":1,\"probe\":\"font-catalog-v1\",\"fonts\":{\"names\":[");
 for(i=0;i<7;i++){memset(name,0xa5,sizeof(name));GetFontName(ids[i],name);p+=sprintf(p,"%s{\"id\":%d,\"name\":",i?",":"",ids[i]);p=text(p,name);p+=sprintf(p,"}");}
 p+=sprintf(p,"],\"numbers\":[");for(i=0;i<9;i++){number=12345;GetFNum(queries[i],&number);p+=sprintf(p,"%s%d",i?",":"",number);}p+=sprintf(p,"],\"menus\":[");
 for(i=0;i<2;i++){menu=NewMenu(21000+i,"\pFonts");AppendMenu(menu,"\pExisting");AppendResMenu(menu,types[i]);items=CountMItems(menu);if(items>128)return 5;p+=sprintf(p,"%s{\"kind\":%lu,\"items\":[",i?",":"",types[i]);for(j=1;j<=items;j++){GetMenuItemText(menu,j,name);p+=sprintf(p,"%s",j>1?",":"");p=text(p,name);}p+=sprintf(p,"]}");DisposeMenu(menu);}p+=sprintf(p,"]}}\n");
 err=Create("\pFontCatalogProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pFontCatalogProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
