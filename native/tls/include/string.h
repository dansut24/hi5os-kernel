#ifndef HI5_STRING_H
#define HI5_STRING_H
#include <stddef.h>
static inline void *memcpy(void *d,const void *s,size_t n){unsigned char *a=d;const unsigned char *b=s;for(size_t i=0;i<n;i++)a[i]=b[i];return d;}
static inline void *memmove(void *d,const void *s,size_t n){unsigned char *a=d;const unsigned char *b=s;if(a<b){for(size_t i=0;i<n;i++)a[i]=b[i];}else{while(n){n--;a[n]=b[n];}}return d;}
static inline void *memset(void *d,int v,size_t n){unsigned char *a=d;for(size_t i=0;i<n;i++)a[i]=(unsigned char)v;return d;}
static inline int memcmp(const void *a,const void *b,size_t n){const unsigned char *x=a,*y=b;for(size_t i=0;i<n;i++)if(x[i]!=y[i])return (int)x[i]-(int)y[i];return 0;}
static inline size_t strlen(const char *s){size_t n=0;while(s[n])n++;return n;}
static inline int strcmp(const char *s,const char *t){while(*s&&*s==*t){s++;t++;}return (unsigned char)*s-(unsigned char)*t;}
static inline int strncmp(const char *s,const char *t,size_t n){while(n&&*s&&*s==*t){s++;t++;n--;}return n?(unsigned char)*s-(unsigned char)*t:0;}
static inline char *strcpy(char *d,const char *s){char *p=d;while((*p++=*s++)){}return d;}
static inline char *strncpy(char *d,const char *s,size_t n){size_t i=0;for(;i<n&&s[i];i++)d[i]=s[i];for(;i<n;i++)d[i]=0;return d;}
static inline void *memchr(const void *s,int c,size_t n){const unsigned char *a=s;for(size_t i=0;i<n;i++)if(a[i]==(unsigned char)c)return (void*)(a+i);return 0;}
static inline char *strchr(const char *s,int c){while(*s){if(*s==(char)c)return (char*)s;s++;}return c==0?(char*)s:0;}
#endif
