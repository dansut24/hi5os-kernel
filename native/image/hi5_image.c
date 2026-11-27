/* Pinned stb_image PNG/JPEG decoder. Bounded freestanding scratch arena.
 * No file I/O, no SIMD, no external heap or scripting. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
static union{uint64_t align;unsigned char bytes[16*1024*1024];} arena;
static size_t used;
static void *alloc_image(size_t n){size_t at=(used+15)&~(size_t)15;if(n>sizeof arena.bytes-at)return 0;used=at+n;return arena.bytes+at;}
static void *resize_image(void *p,size_t old,size_t n){void *q=alloc_image(n);if(q&&p)memcpy(q,p,old<n?old:n);return q;}
#define STB_IMAGE_IMPLEMENTATION
#define STBI_NO_STDIO
#define STBI_NO_SIMD
#define STBI_NO_HDR
#define STBI_NO_LINEAR
#define STBI_ONLY_PNG
#define STBI_ONLY_JPEG
#define STBI_MAX_DIMENSIONS 2048
#define STBI_MALLOC(n) alloc_image(n)
#define STBI_REALLOC_SIZED(p,o,n) resize_image(p,o,n)
#define STBI_FREE(p) ((void)(p))
#define STBI_ASSERT(x) ((void)0)
#include "stb_image.h"
int hi5_image_decode(const unsigned char *data,size_t len,unsigned char *out,size_t cap,uint32_t *ow,uint32_t *oh){
    int w,h,n;used=0;*ow=*oh=0;
    if(!len||len>4*1024*1024||!stbi_info_from_memory(data,(int)len,&w,&h,&n)||w<=0||h<=0||w>2048||h>2048)return 0;
    used=0;unsigned char *p=stbi_load_from_memory(data,(int)len,&w,&h,&n,4);
    if(!p)return 0;unsigned int dw=(unsigned)w,dh=(unsigned)h;
    if(dw>320){dh=dh*320/dw;dw=320;}if(dh>180){dw=dw*180/dh;dh=180;}if(!dw)dw=1;if(!dh)dh=1;
    if((size_t)dw*dh*3>cap)return 0;
    for(unsigned int y=0;y<dh;y++)for(unsigned int x=0;x<dw;x++){
        const unsigned char *s=p+((size_t)(y*(unsigned)h/dh)*(unsigned)w+x*(unsigned)w/dw)*4;unsigned char *d=out+((size_t)y*dw+x)*3;
        for(int k=0;k<3;k++)d[k]=(unsigned char)((s[k]*s[3]+255*(255-s[3])+127)/255);
    }*ow=dw;*oh=dh;return 1;
}
