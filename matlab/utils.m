function u = utils()
     u.plot_samples = @plot_samples;
     u.plot_stdin = @plot_stdin;
end

function plot_samples(path) 
     S = readmatrix(path);
     m = size(S, 1);
     max(S)
     t = linspace(1, m, m);
 
     plot(t, S);
end

function plot_stdin()
     S = readmatrix(path);
     m = size(S, 1);
     max(S)
     t = linspace(1, m, m);

     
end