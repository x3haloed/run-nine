/* Read AppearanceLib's real configuration through the Mixed Mode Manager. */
#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
typedef long (*ThemeFontFunction)(long,long,void*,void*,void*);
static char out[32768];
typedef Ptr (*MakeStringFunction)(Ptr,Ptr,long,long,long);
typedef long (*DimensionsFunction)(Ptr,long,long,long,Point*,short*);
typedef void (*ReleaseFunction)(Ptr);
static UniversalProcPtr get_proc(ConnectionID connection,ConstStr255Param name,short count,short result,OSErr *err){
 Ptr symbol=0;SymClass kind;short i;ProcInfoType info=kCStackBased|((ProcInfoType)result<<4);UniversalProcPtr p=0;
 *err=FindSymbol(connection,name,&symbol,&kind);if(*err)return 0;
 for(i=1;i<=count;i++)info|=STACK_ROUTINE_PARAMETER(i,kFourByteCode);
 p=NewRoutineDescriptor((ProcPtr)symbol,info,kPowerPCISA);if(!p)*err=memFullErr;return p;
}
static char *carbon_metrics(char *p){
 ConnectionID connection=0;Ptr main=0;Str255 error;OSErr load,lookup[3]={-1,-1,-1};
 UniversalProcPtr create=0,dimensions=0,release=0;short i,j;long result;Point bounds;short baseline;Ptr text;
 char *samples[]={"One","Renamed","Reference","Untitled","AV","To","WA","d","dd","Rd","dR","R"};
 memset(error,0,sizeof(error));load=GetSharedLibrary("\pCarbonLib",'pwpc',1,&connection,&main,error);
 if(!load){create=get_proc(connection,"\pCFStringCreateWithBytes",5,kFourByteCode,&lookup[0]);dimensions=get_proc(connection,"\pGetThemeTextDimensions",6,kFourByteCode,&lookup[1]);release=get_proc(connection,"\pCFRelease",1,kNoByteCode,&lookup[2]);}
 p+=sprintf(p,"\"carbon\":{\"load_error\":%d,\"lookup_errors\":[%d,%d,%d],\"roles\":[",load,lookup[0],lookup[1],lookup[2]);
 if(create&&dimensions&&release)for(i=0;i<2;i++){
  long role=i?104:0;p+=sprintf(p,"%s{\"role\":%ld,\"glyphs\":[",i?",":"",role);
  for(j=0;j<256;j++){
   unsigned char byte=j;bounds.v=bounds.h=baseline=0;
   text=((MakeStringFunction)create)(0,(Ptr)&byte,1,0,0);result=memFullErr;
   if(text){result=((DimensionsFunction)dimensions)(text,role,1,0,&bounds,&baseline);((ReleaseFunction)release)(text);}
   p+=sprintf(p,"%s[%ld,%d,%d,%d]",j?",":"",result,bounds.h,bounds.v,baseline);
  }
  p+=sprintf(p,"],\"samples\":[");
  for(j=0;j<12;j++){
   bounds.v=bounds.h=baseline=0;text=((MakeStringFunction)create)(0,samples[j],strlen(samples[j]),0,0);result=memFullErr;
   if(text){result=((DimensionsFunction)dimensions)(text,role,1,0,&bounds,&baseline);((ReleaseFunction)release)(text);}
   p+=sprintf(p,"%s[%ld,%d,%d,%d]",j?",":"",result,bounds.h,bounds.v,baseline);
  }
  p+=sprintf(p,"]}");
 }
 p+=sprintf(p,"]}");if(create)DisposeRoutineDescriptor(create);if(dimensions)DisposeRoutineDescriptor(dimensions);if(release)DisposeRoutineDescriptor(release);if(!load)CloseConnection(&connection);return p;
}
static char *port_metrics(char *p,char *label,GrafPtr port){
 short c;Str255 name;Point unit={1,1};GetFontName(port->txFont,name);
 SetPort(port);
 p+=sprintf(p,"\"%s\":{\"font\":%d,\"size\":%d,\"face\":%u,\"mode\":%d,\"extra\":%ld,\"outline\":%d,\"preferred\":%d,\"font_name_bytes\":[",label,port->txFont,port->txSize,(unsigned char)port->txFace,port->txMode,port->spExtra,IsOutline(unit,unit),GetOutlinePreferred());
 for(c=0;c<name[0];c++)p+=sprintf(p,"%s%u",c?",":"",name[c+1]);
 p+=sprintf(p,"],\"widths\":[");for(c=0;c<256;c++)p+=sprintf(p,"%s%d",c?",":"",CharWidth(c));
 p+=sprintf(p,"]}");return p;
}
int main(void){
 ConnectionID connection=0;Ptr main=0,symbol=0;SymClass kind=0;Str255 error,name;
 UniversalProcPtr routine=0;short i,f,size,font;Style face;long n,status;OSErr load,lookup=-1,err;char *p=out;
 ProcInfoType info=kCStackBased | (kFourByteCode<<4) | STACK_ROUTINE_PARAMETER(1,kFourByteCode) | STACK_ROUTINE_PARAMETER(2,kFourByteCode) | STACK_ROUTINE_PARAMETER(3,kFourByteCode) | STACK_ROUTINE_PARAMETER(4,kFourByteCode) | STACK_ROUTINE_PARAMETER(5,kFourByteCode);
 short roles[]={0,1,2,3,4,104};GrafPort port;GrafPtr mono;CGrafPtr color;WindowPtr window;Rect wr={60,50,180,250};
 InitGraf(&qd.thePort);InitFonts();InitWindows();OpenPort(&port);
 load=GetSharedLibrary("\pAppearanceLib",'pwpc',1,&connection,&main,error);
 if(!load){lookup=FindSymbol(connection,"\pGetThemeFont",&symbol,&kind);if(!lookup)routine=NewRoutineDescriptor((ProcPtr)symbol,info,kPowerPCISA);}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"theme-font-v1\",\"load_error\":%d,\"lookup_error\":%d,\"symbol_class\":%u,\"proc_info\":%lu,\"descriptor\":%d,\"roles\":[",load,lookup,kind,info,routine!=0);
 if(routine)for(i=0;i<6;i++){
  memset(name,0,sizeof(name));size=-1;face=0;
  status=((ThemeFontFunction)routine)(roles[i],0,name,&size,&face);
  {unsigned short length=name[0];if(length>200)length=200;memmove(name,name+1,length);name[length]=0;}
  font=-1;if(!status){Str255 pn;pn[0]=strlen((char*)name);memcpy(pn+1,name,pn[0]);GetFNum(pn,&font);}
  p+=sprintf(p,"%s{\"role\":%d,\"error\":%ld,\"name\":\"%s\",\"id\":%d,\"size\":%d,\"face\":%u}",i?",":"",roles[i],status,name,font,size,face);
 }
 p+=sprintf(p,"],\"ports\":{");GetWMgrPort(&mono);GetCWMgrPort(&color);
 window=NewWindow(0,&wr,"\pReference",false,documentProc,(WindowPtr)-1,true,0);
 p=port_metrics(p,"mono",mono);p+=sprintf(p,",");p=port_metrics(p,"color",(GrafPtr)color);
 SetWTitle(window,"\pReference");p+=sprintf(p,"},\"set_title_width\":%d,\"sys_font\":[%d,%d],\"outline_preferred_widths\":[",((WindowPeek)window)->titleWidth,*(short*)0xba6,*(short*)0xba8);
 SetPort(&port);TextFont(0);TextSize(12);TextFace(0);SetOutlinePreferred(true);
 for(i=0;i<256;i++)p+=sprintf(p,"%s%d",i?",":"",CharWidth(i));
 p+=sprintf(p,"],");p=carbon_metrics(p);p+=sprintf(p,"}\n");SetOutlinePreferred(false);DisposeWindow(window);if(routine)DisposeRoutineDescriptor(routine);if(!load)CloseConnection(&connection);ClosePort(&port);
 err=Create("\pThemeFontProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pThemeFontProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
