/* Offline HFS image access through libhfs. Never use on a mounted guest disk. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include "hfs.h"
static void fail(const char *what) {
    fprintf(stderr, "%s: %s\n", what, hfs_error ? hfs_error : "I/O error");
    exit(1);
}
int main(int argc, char **argv) {
    hfsvol *vol;
    if (argc < 3) { fprintf(stderr,"Usage: hfs-image create IMAGE LABEL | ls IMAGE [PATH] | get IMAGE PATH FORK OUT\n"); return 2; }
    if (!strcmp(argv[1],"create")) {
        if (argc != 4) return 2;
        int fd=open(argv[2],O_CREAT|O_EXCL|O_RDWR,0600);
        if(fd<0) { perror("create (refuses existing file)"); return 1; }
        if(ftruncate(fd,16*1024*1024)<0) { perror("ftruncate"); close(fd); return 1; }
        close(fd);
        if(hfs_format(argv[2],0,HFS_MODE_RDWR,argv[3],0,NULL)<0) fail("format");
        return 0;
    }
    vol=hfs_mount(argv[2],0,!strcmp(argv[1],"put-resource") ? HFS_MODE_RDWR : HFS_MODE_RDONLY);
    if(!vol) fail("mount read-only");
    if(!strcmp(argv[1],"put-resource")) {
        if(argc!=7 || strlen(argv[5])!=4 || strlen(argv[6])!=4) return 2;
        hfsdirent existing;
        if(hfs_stat(vol,argv[3],&existing)==0) { fprintf(stderr,"Refusing to overwrite guest file\n"); return 1; }
        FILE *in=fopen(argv[4],"rb");
        if(!in) { perror("open input"); return 1; }
        hfsfile *file=hfs_create(vol,argv[3],argv[5],argv[6]);
        if(!file) fail("create guest file");
        if(hfs_setfork(file,1)<0) fail("resource fork");
        unsigned char buffer[8192]; size_t n;
        while((n=fread(buffer,1,sizeof buffer,in))>0)
            if(hfs_write(file,buffer,n)!=n) fail("write resource fork");
        if(ferror(in)) { perror("read input"); return 1; }
        fclose(in); hfs_close(file);
    } else if(!strcmp(argv[1],"ls")) {
        hfsdir *dir=hfs_opendir(vol,argc>3?argv[3]:":");
        hfsdirent entry;
        if(!dir) fail("opendir");
        while(hfs_readdir(dir,&entry)==0) {
            if(entry.flags&HFS_ISDIR) printf("DIR\t%s\n",entry.name);
            else printf("%.4s\t%.4s\t%lu\t%lu\t%s\n",entry.u.file.type,entry.u.file.creator,entry.u.file.dsize,entry.u.file.rsize,entry.name);
        }
        hfs_closedir(dir);
    } else if(!strcmp(argv[1],"get")) {
        if(argc!=6 || (strcmp(argv[4],"data") && strcmp(argv[4],"resource"))) return 2;
        hfsfile *file=hfs_open(vol,argv[3]);
        if(!file) fail("open guest file");
        if(hfs_setfork(file,!strcmp(argv[4],"resource"))<0) fail("fork");
        FILE *out=fopen(argv[5],"wb");
        if(!out) { perror("open host output"); return 1; }
        unsigned char buffer[8192]; unsigned long n;
        while((n=hfs_read(file,buffer,sizeof buffer))>0) {
            if(n==(unsigned long)-1) fail("read");
            if(fwrite(buffer,1,n,out)!=n) { perror("write"); return 1; }
        }
        fclose(out); hfs_close(file);
    } else { fprintf(stderr,"Unknown action\n"); return 2; }
    if(hfs_umount(vol)<0) fail("unmount");
    return 0;
}
