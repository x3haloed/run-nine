#include <Memory.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static Handle allocate(long size,long *error,unsigned char *zero,unsigned char *negative){
    register long d0 __asm__("d0")=size;
    register Handle a0 __asm__("a0");
    unsigned char z,n;
    __asm__ volatile(".word 0xa122\n\tseq %2\n\tsmi %3" : "+d"(d0),"=a"(a0),"=d"(z),"=d"(n) : : "cc","memory");
    *error=d0;*zero=z!=0;*negative=n!=0;return a0;
}
static long handle_size(Handle handle,unsigned char *zero,unsigned char *negative){
    register long d0 __asm__("d0");
    register Handle a0 __asm__("a0")=handle;
    unsigned char z,n;
    __asm__ volatile(".word 0xa025\n\tseq %2\n\tsmi %3" : "=d"(d0),"+a"(a0),"=d"(z),"=d"(n) : : "cc","memory");
    *zero=z!=0;*negative=n!=0;return d0;
}
int main(void){
    char out[1024];short file;long count,error,small_size,large_size,failure;
    unsigned char sz,sn,ssz,ssn,lz,ln,lsz,lsn,fz,fn;Handle small,large,failed;OSErr err;
    small=allocate(16,&error,&sz,&sn);if(!small)return 5;
    small_size=handle_size(small,&ssz,&ssn);
    large=allocate(65536,&error,&lz,&ln);if(!large)return 6;
    large_size=handle_size(large,&lsz,&lsn);
    failed=allocate(0x7fffffffL,&failure,&fz,&fn);
    sprintf(out,"{\"schema\":1,\"probe\":\"memory-flags-v1\",\"flags\":{"
      "\"small_allocation\":[%d,%d],\"small_size\":[%ld,%d,%d],\"large_allocation\":[%d,%d],"
      "\"large_size\":[%ld,%d,%d],\"failure\":[%ld,%d,%d],\"failure_null\":%d}}\n",
      sz,sn,small_size,ssz,ssn,lz,ln,large_size,lsz,lsn,failure,fz,fn,failed==0);
    DisposeHandle(small);DisposeHandle(large);if(failed)DisposeHandle(failed);
    err=Create("\pMemoryFlagsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pMemoryFlagsProbe.json",0,&file))return 3;
    SetEOF(file,0);count=strlen(out);err=FSWrite(file,&count,out);FSClose(file);FlushVol(0,0);return err?4:0;
}
