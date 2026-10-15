#import <AVFoundation/AVFoundation.h>
#import <CoreText/CoreText.h>
#import <ImageIO/ImageIO.h>
#import <Foundation/Foundation.h>

static void label(CGContextRef c, NSString *text, CGFloat x, CGFloat y, CGFloat size, CGColorRef color) {
    CTFontRef font = CTFontCreateWithName(CFSTR("HelveticaNeue-Medium"), size, NULL);
    NSAttributedString *string = [[NSAttributedString alloc] initWithString:text attributes:@{(__bridge id)kCTFontAttributeName:(__bridge id)font, (__bridge id)kCTForegroundColorAttributeName:(__bridge id)color}];
    CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)string);
    CGContextSaveGState(c); CGContextTranslateCTM(c,x,y+size); CGContextScaleCTM(c,1,-1); CGContextSetTextPosition(c,0,0); CTLineDraw(line,c); CGContextRestoreGState(c);
    CFRelease(line); CFRelease(font);
}
static void limb(CGContextRef c, CGPoint *points, size_t count, CGColorRef color, CGFloat width) {
    CGContextSetStrokeColorWithColor(c,color); CGContextSetLineWidth(c,width); CGContextSetLineCap(c,kCGLineCapRound); CGContextSetLineJoin(c,kCGLineJoinRound);
    CGContextBeginPath(c); CGContextMoveToPoint(c,points[0].x,points[0].y);
    for(size_t i=1;i<count;i++) CGContextAddLineToPoint(c,points[i].x,points[i].y);
    CGContextStrokePath(c);
}
static CGColorRef color(CGFloat r,CGFloat g,CGFloat b) { return CGColorCreateGenericRGB(r,g,b,1); }
int main(int argc,const char **argv) { @autoreleasepool {
    if(argc!=2)return 2;
    NSURL *url=[NSURL fileURLWithPath:@(argv[1])]; [[NSFileManager defaultManager] removeItemAtURL:url error:nil];
    NSError *error=nil; AVAssetWriter *writer=[[AVAssetWriter alloc] initWithURL:url fileType:AVFileTypeMPEG4 error:&error];
    AVAssetWriterInput *input=[AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeVideo outputSettings:@{AVVideoCodecKey:AVVideoCodecTypeH264,AVVideoWidthKey:@640,AVVideoHeightKey:@360,AVVideoCompressionPropertiesKey:@{AVVideoAverageBitRateKey:@550000,AVVideoProfileLevelKey:AVVideoProfileLevelH264BaselineAutoLevel}}];
    AVAssetWriterInputPixelBufferAdaptor *adaptor=[AVAssetWriterInputPixelBufferAdaptor assetWriterInputPixelBufferAdaptorWithAssetWriterInput:input sourcePixelBufferAttributes:@{(id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32ARGB),(id)kCVPixelBufferWidthKey:@640,(id)kCVPixelBufferHeightKey:@360,(id)kCVPixelBufferCGImageCompatibilityKey:@YES,(id)kCVPixelBufferCGBitmapContextCompatibilityKey:@YES}];
    [writer addInput:input]; [writer startWriting]; [writer startSessionAtSourceTime:kCMTimeZero];
    CGColorRef green=color(.185,.757,.525),ink=color(.91,.96,.93),background=color(.07,.11,.095),shadow=color(.1,.16,.135),farColor=color(.22,.37,.3),muted=color(.65,.65,.65);
    for(int i=0;i<180;i++) { @autoreleasepool {
        while(!input.readyForMoreMediaData)[NSThread sleepForTimeInterval:.002];
        CVPixelBufferRef buffer=NULL; CVPixelBufferPoolCreatePixelBuffer(NULL,adaptor.pixelBufferPool,&buffer); CVPixelBufferLockBaseAddress(buffer,0);
        CGColorSpaceRef space=CGColorSpaceCreateDeviceRGB(); CGContextRef c=CGBitmapContextCreate(CVPixelBufferGetBaseAddress(buffer),640,360,8,CVPixelBufferGetBytesPerRow(buffer),space,kCGImageAlphaNoneSkipFirst);CGColorSpaceRelease(space);
        CGContextTranslateCTM(c,0,360);CGContextScaleCTM(c,1,-1);CGContextSetFillColorWithColor(c,background);CGContextFillRect(c,CGRectMake(0,0,640,360));
        CGContextSetFillColorWithColor(c,shadow);CGContextFillEllipseInRect(c,CGRectMake(250,316,210,12));
        double phase=(double)i/180; CGFloat bend=(1-cos(phase*M_PI*2))/2;
        CGPoint ankle=CGPointMake(365,309),knee=CGPointMake(365+30*bend,225+31*bend),hip=CGPointMake(360-65*bend,145+91*bend);
        CGPoint shoulder=CGPointMake(hip.x-5-25*bend,hip.y-88+8*bend),elbow=CGPointMake(shoulder.x+52*bend,shoulder.y+42-15*bend),hand=CGPointMake(shoulder.x+100*bend,shoulder.y+80-65*bend);
        CGPoint far[]={CGPointMake(hip.x-15,hip.y),CGPointMake(knee.x-14,knee.y),CGPointMake(ankle.x-18,ankle.y)};limb(c,far,3,farColor,17);
        CGPoint body[]={ankle,knee,hip,shoulder};limb(c,body,4,ink,18);
        CGPoint foot[]={CGPointMake(ankle.x-6,ankle.y+3),CGPointMake(ankle.x+26,ankle.y+3)};limb(c,foot,2,green,12);
        CGPoint arm[]={shoulder,elbow,hand};limb(c,arm,3,green,12);
        CGContextSetFillColorWithColor(c,ink);CGContextFillEllipseInRect(c,CGRectMake(shoulder.x-12,shoulder.y-40,35,35));CGContextSetFillColorWithColor(c,green);CGContextFillEllipseInRect(c,CGRectMake(hip.x-6,hip.y-6,12,12));
        label(c,@"SQUAT",32,44,30,green);label(c,@"Side view",34,86,17,ink);label(c,i<90?@"Slowly lower":@"Stand tall",34,249,18,ink);label(c,@"Move at your own pace",34,277,12,muted);
        CGContextSetFillColorWithColor(c,shadow);CGContextFillRect(c,CGRectMake(34,319,150,3));CGContextSetFillColorWithColor(c,green);CGContextFillRect(c,CGRectMake(34,319,150*phase,3));
        if(i==90){CGImageRef image=CGBitmapContextCreateImage(c);NSURL *snapshot=[NSURL fileURLWithPath:[url.path stringByAppendingString:@".png"]];CGImageDestinationRef dest=CGImageDestinationCreateWithURL((__bridge CFURLRef)snapshot,CFSTR("public.png"),1,NULL);CGImageDestinationAddImage(dest,image,NULL);CGImageDestinationFinalize(dest);CFRelease(dest);CGImageRelease(image);}
        CGContextRelease(c);CVPixelBufferUnlockBaseAddress(buffer,0);
        BOOL ok=[adaptor appendPixelBuffer:buffer withPresentationTime:CMTimeMake(i,30)];CVPixelBufferRelease(buffer);if(!ok){NSLog(@"%@",writer.error);return 1;}
    }}
    [input markAsFinished];dispatch_semaphore_t semaphore=dispatch_semaphore_create(0);[writer finishWritingWithCompletionHandler:^{dispatch_semaphore_signal(semaphore);}];dispatch_semaphore_wait(semaphore,DISPATCH_TIME_FOREVER);
    if(writer.status!=AVAssetWriterStatusCompleted){NSLog(@"%@",writer.error);return 1;}printf("Created original squat demonstration: %s\n",argv[1]);
    CGColorRelease(green);CGColorRelease(ink);CGColorRelease(background);CGColorRelease(shadow);CGColorRelease(farColor);CGColorRelease(muted);
    return 0;
}}
