typedef unsigned long long U64;
typedef unsigned int U32;
typedef unsigned short U16;
typedef struct { U32 style; U64 proc; int cls, wnd; U64 instance, icon, cursor, brush, menu, name; } WNDCLASS;
typedef struct { U64 hwnd; U32 message; U64 wparam,lparam; U32 time; int x,y; U32 priv; } MSG;
__declspec(dllimport) int Hi5Answer(void);
__declspec(dllimport) U16 RegisterClassW(const WNDCLASS *);
__declspec(dllimport) U64 CreateWindowExW(U32,const U16*,const U16*,U32,int,int,int,int,U64,U64,U64,U64);
__declspec(dllimport) int ShowWindow(U64,int);
__declspec(dllimport) U64 GetDC(U64);
__declspec(dllimport) int ReleaseDC(U64,U64);
__declspec(dllimport) int Rectangle(U64,int,int,int,int);
__declspec(dllimport) int PostMessageW(U64,U32,U64,U64);
__declspec(dllimport) int PeekMessageW(MSG*,U64,U32,U32,U32);
static U64 window_proc(U64 hwnd,U32 msg,U64 w,U64 l){return hwnd+msg+w+l;}
static const U16 name[]={72,105,53,71,117,105,0};
static const U16 title[]={72,105,53,32,80,69,0};
static const WNDCLASS cls={0,(U64)window_proc,0,0,0,0,0,0,0,(U64)name};
int hi5_entry(void){
    if(Hi5Answer()!=42||!RegisterClassW(&cls))return 1;
    U64 h=CreateWindowExW(0,name,title,0,0,0,240,120,0,0,0,0);
    if(!h)return 2;
    ShowWindow(h,5);
    U64 dc=GetDC(h);if(!dc||!Rectangle(dc,8,8,180,78)||!ReleaseDC(h,dc))return 3;
    if(!PostMessageW(h,0x400,123,456))return 4;
    MSG m;
    while(PeekMessageW(&m,0,0,0,1)){if(m.message==0x400)return m.hwnd==h&&m.wparam==123&&m.lparam==456?42:5;}
    return 6;
}
