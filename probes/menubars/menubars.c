#include <QuickDraw.h>
#include <Menus.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void){MenuHandle old,a,b,again;Handle base,bar,snapshot,second,missing,empty,partial;char output[4096],*p=output;short file;long count;OSErr err,resource_error;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();
 old=NewMenu(20999,"\pOld");AppendMenu(old,"\pEarlier/E");InsertMenu(old,0);base=GetMenuBar();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"menu-bars-v1\",\"menus\":{");
 bar=GetNewMBar(21000);resource_error=ResError();p+=sprintf(p,"\"new\":[%d,%d,%d,%d,%lu]",bar!=0,resource_error,GetMenuHandle(20999)==old,GetMenuHandle(21000)==0,MenuKey('e'));
 if(!bar)return 5;SetMenuBar(bar);a=GetMenuHandle(21000);b=GetMenuHandle(21001);again=GetMenu(21000);
 p+=sprintf(p,",\"installed\":[%d,%d,%d,%d,%lu,%lu]",a!=0,b!=0,GetMenuHandle(20999)==0,again==a,MenuKey('n'),MenuKey('c'));
 snapshot=GetMenuBar();second=GetMenuBar();p+=sprintf(p,",\"snapshots\":[%d,%d,%d,%d]",snapshot!=bar,second!=snapshot,GetMenuHandle(21000)==a,GetResource('MENU',21000)==(Handle)a);DisposeHandle(second);
 DeleteMenu(21000);p+=sprintf(p,",\"deleted\":[%d,%lu,%lu]",GetMenuHandle(21000)==0,MenuKey('n'),MenuKey('c'));
 SetMenuBar(snapshot);DisposeHandle(snapshot);p+=sprintf(p,",\"restored\":[%d,%d,%lu]",GetMenuHandle(21000)==a,GetMenuHandle(21001)==b,MenuKey('n'));
 SetMenuBar(base);DisposeHandle(bar);p+=sprintf(p,",\"base\":[%d,%d,%lu]",GetMenuHandle(20999)==old,GetMenuHandle(21000)==0,MenuKey('e'));
 missing=GetNewMBar(31000);resource_error=ResError();p+=sprintf(p,",\"missing\":[%d,%d,%d]",missing==0,resource_error,GetMenuHandle(20999)==old);
 empty=GetNewMBar(21001);resource_error=ResError();p+=sprintf(p,",\"empty\":[%d,%d]",empty!=0,resource_error);if(empty){SetMenuBar(empty);p+=sprintf(p,",\"empty_installed\":[%d,%lu]",GetMenuHandle(20999)==0,MenuKey('e'));SetMenuBar(base);DisposeHandle(empty);}
 partial=GetNewMBar(21002);resource_error=ResError();p+=sprintf(p,",\"partial\":[%d,%d,%d]",partial!=0,resource_error,GetMenuHandle(20999)==old);if(partial){SetMenuBar(partial);p+=sprintf(p,",\"partial_installed\":[%d,%d,%d,%lu]",GetMenuHandle(21000)!=0,GetMenuHandle(21001)!=0,GetMenuHandle(30000)==0,MenuKey('n'));SetMenuBar(base);DisposeHandle(partial);}
 p+=sprintf(p,"}}\n");err=Create("\pMenuBarProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pMenuBarProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
