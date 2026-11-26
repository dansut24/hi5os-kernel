static volatile int answer=42;
static volatile int *volatile pointer=&answer;
__declspec(dllexport) int Hi5Answer(void){return *pointer;}
