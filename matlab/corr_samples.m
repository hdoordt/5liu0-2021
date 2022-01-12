function corr = corr_samples(ref, sig, Ts, len, skip)

    if(exist('skip', 'var'))
        ref = ref(skip+1:end);
        sig = sig(skip+1:end);
    end

    if(exist('len', 'var') && len ~= -1)
        ref = ref(1: len);
        sig = sig(1: len);
    end
    
    m = length(ref);
    
    n = linspace(-m, m, m*2-1);
    [res, lags] = xcorr(ref, sig);

    corr.t = n * Ts;
    corr.n = n;
    corr.corr = res;
    corr.lags = lags;
end
